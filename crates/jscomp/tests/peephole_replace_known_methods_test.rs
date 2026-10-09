/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PeepholeReplaceKnownMethodsTest.java.

//! Port of `PeepholeReplaceKnownMethodsTest.java`: unit tests for PeepholeReplaceKnownMethods.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_replace_known_methods::PeepholeReplaceKnownMethods,
};
use closure_rhino::fast_hash::IndexMap;
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

// port: PeepholeReplaceKnownMethodsTest#PeepholeReplaceKnownMethodsTest (externs suffix)
const EXTERNS_SUFFIX: &str = "/** @type {function(this: Array, ...*): !Array<?>} */ function returnArrayType() {}\n/** @type {function(this: Array, ...*): !Array<?>|string} */ function returnUnionType(){}\n/** @constructor */ function Foo(){}\n/** @type {function(this: Foo, ...*): !Foo} */ Foo.prototype.concat\nvar obj = new Foo();\n/**\n * @param {...T} var_args\n * @return {!Array<T>}\n * @template T\n */\nArray.of = function(var_args) {};\n";

struct PeepholeReplaceKnownMethodsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    late: bool,
    use_types: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeReplaceKnownMethodsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let late = self.late;
        let use_types = self.use_types;
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
                    vec![Box::new(PeepholeReplaceKnownMethods::new(late, use_types))];
                let mut peephole_pass = PeepholeOptimizationsPass::new(name.clone(), optimizations);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeReplaceKnownMethodsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeReplaceKnownMethodsTest {
    // port: PeepholeReplaceKnownMethodsTest#PeepholeReplaceKnownMethodsTest
    // port: PeepholeReplaceKnownMethodsTest#setUp
    fn new() -> Self {
        let minimal_externs = MINIMAL_EXTERNS.clone().unwrap();
        let mut externs = minimal_externs.as_units().to_vec();
        externs.extend(EXTERNS_SUFFIX.encode_utf16());
        let mut harness = CompilerTestCase::new(JsString::from_units(externs));
        harness.set_up();
        let late = true;
        let use_types = true;
        harness.disable_type_check().unwrap();
        harness.enable_normalize().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeReplaceKnownMethodsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                late,
                use_types,
            },
        }
    }

    fn test(&mut self, js: impl Into<JsString>, expected: impl Into<JsString>) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same(&mut self, js: impl Into<JsString>) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    // port: PeepholeReplaceKnownMethodsTest#foldSame
    fn fold_same(&mut self, js: impl Into<JsString>) {
        self.test_same(js);
    }

    // port: PeepholeReplaceKnownMethodsTest#fold
    fn fold(&mut self, js: impl Into<JsString>, expected: impl Into<JsString>) {
        self.test(js, expected);
    }

    // port: PeepholeReplaceKnownMethodsTest#foldSameStringTyped
    fn fold_same_string_typed(&mut self, js: &str) {
        self.fold_string_typed(js, js);
    }

    // port: PeepholeReplaceKnownMethodsTest#foldStringTyped
    fn fold_string_typed(&mut self, js: &str, expected: &str) {
        self.test(
            format!("function f(/** string */ a) {{{js}}}"),
            format!("function f(/** string */ a) {{{expected}}}"),
        );
    }
}

/// Java string `pre + (char) unit + post`: Rust `&str` cannot hold the unpaired surrogate the Java
/// test literals contain.
fn with_unit(pre: &str, unit: u16, post: &str) -> JsString {
    let mut units: Vec<u16> = pre.encode_utf16().collect();
    units.push(unit);
    units.extend(post.encode_utf16());
    JsString::from_units(units)
}

fn language_mode(name: &str) -> DslValue {
    DslValue::Enum {
        class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
        name: name.into(),
    }
}

// port: PeepholeReplaceKnownMethodsTest#testStringIndexOf
#[test]
fn test_string_index_of() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcdef'.indexOf('g')", "x = -1");
    t.fold("x = 'abcdef'.indexOf('b')", "x = 1");
    t.fold("x = 'abcdefbe'.indexOf('b', 2)", "x = 6");
    t.fold("x = 'abcdef'.indexOf('bcd')", "x = 1");
    t.fold("x = 'abcdefsdfasdfbcdassd'.indexOf('bcd', 4)", "x = 13");
    t.fold("x = 'abcdef'.lastIndexOf('b')", "x = 1");
    t.fold("x = 'abcdefbe'.lastIndexOf('b')", "x = 6");
    t.fold("x = 'abcdefbe'.lastIndexOf('b', 5)", "x = 1");
    // Both elements must be strings. Don't do anything if either one is not
    // string.
    t.fold("x = 'abc1def'.indexOf(1)", "x = 3");
    t.fold("x = 'abcNaNdef'.indexOf(NaN)", "x = 3");
    t.fold("x = 'abcundefineddef'.indexOf(undefined)", "x = 3");
    t.fold("x = 'abcnulldef'.indexOf(null)", "x = 3");
    t.fold("x = 'abctruedef'.indexOf(true)", "x = 3");
    // The following test case fails with JSC_PARSE_ERROR. Hence omitted.
    // foldSame("x = 1.indexOf('bcd');");
    t.fold_same("x = NaN.indexOf('bcd')");
    t.fold_same("x = undefined.indexOf('bcd')");
    t.fold_same("x = null.indexOf('bcd')");
    t.fold_same("x = true.indexOf('bcd')");
    t.fold_same("x = false.indexOf('bcd')");
    // Avoid dealing with regex or other types.
    t.fold_same("x = 'abcdef'.indexOf(/b./)");
    t.fold_same("x = 'abcdef'.indexOf({a:2})");
    t.fold_same("x = 'abcdef'.indexOf([1,2])");
    // Template Strings
    t.fold_same("x = `abcdef`.indexOf('b')");
    t.fold_same("x = `Hello ${name}`.indexOf('a')");
    t.fold_same("x = tag `Hello ${name}`.indexOf('a')");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringIncludes
#[test]
fn test_fold_string_includes() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold String.prototype.includes with Constant Arguments
    // Baseline current behavior and guards:
    // Under ECMA-262 § 22.1.3.8, String.prototype.includes evaluates substring search with position
    // clamping.
    // Future optimization fold targets:
    // - 'hello world'.includes('world') -> true
    // - 'foo'.includes('bar') -> false
    // - 'abcdef'.includes('bc') -> true
    // - 'abcdef'.includes('xyz') -> false
    // - 'abc'.includes('') -> true
    // - ''.includes('') -> true
    // - 'abc'.includes('a', 1) -> false
    // - 'abc'.includes('b', 1) -> true
    // - 'abc'.includes('c', 2) -> true
    // - 'abcdef'.includes('bc', 1) -> true
    // - 'abcdef'.includes('bc', 2) -> false
    // - 'abc'.includes('a', -5) -> true
    // - 'abc'.includes('a', 10) -> false
    // - 'abcdef'.includes('bc', -5) -> true
    // - 'abcdef'.includes('bc', 100) -> false
    // - '123'.includes(2) -> true
    // - 'true'.includes(true) -> true
    // - 'abc1def'.includes(1) -> true
    // - 'abctruedef'.includes(true) -> true
    // - 'abcnulldef'.includes(null) -> true
    // - 'abcundefineddef'.includes(undefined) -> true
    // - 'abcNaNdef'.includes(NaN) -> true
    t.fold_same("x = 'hello world'.includes('world')");
    t.fold_same("x = 'foo'.includes('bar')");
    t.fold_same("x = 'abcdef'.includes('bc')");
    t.fold_same("x = 'abcdef'.includes('xyz')");
    t.fold_same("x = 'abc'.includes('')");
    t.fold_same("x = ''.includes('')");
    // Positional search & clamping
    t.fold_same("x = 'abc'.includes('a', 1)");
    t.fold_same("x = 'abc'.includes('b', 1)");
    t.fold_same("x = 'abc'.includes('c', 2)");
    t.fold_same("x = 'abcdef'.includes('bc', 1)");
    t.fold_same("x = 'abcdef'.includes('bc', 2)");
    t.fold_same("x = 'abc'.includes('a', -5)");
    t.fold_same("x = 'abc'.includes('a', 10)");
    t.fold_same("x = 'abcdef'.includes('bc', -5)");
    t.fold_same("x = 'abcdef'.includes('bc', 100)");
    // Coercions
    t.fold_same("x = '123'.includes(2)");
    t.fold_same("x = 'true'.includes(true)");
    t.fold_same("x = 'abc1def'.includes(1)");
    t.fold_same("x = 'abctruedef'.includes(true)");
    t.fold_same("x = 'abcnulldef'.includes(null)");
    t.fold_same("x = 'abcundefineddef'.includes(undefined)");
    t.fold_same("x = 'abcNaNdef'.includes(NaN)");
    // Negative / Guard cases (Must NOT fold)
    t.fold_same("x = str.includes('a')");
    // non-literal receiver
    t.fold_same("x = 'abc'.includes(y)");
    // non-constant argument
    t.fold_same("x = 'abc'.includes((foo(), 'a'))");
    // side-effecting argument
    t.fold_same("x = 'abc'.includes(/a/)");
    // regex argument throws TypeError at runtime (ECMA-262 §
    // 22.1.3.8)
    t.fold_same("x = 'abcdef'.includes(/bc/)");
    t.fold_same("x = 'abcdef'.includes('bc', pos)");
    // non-constant position
    t.fold_same("x = 'abcdef'.includes('bc', 1, 2)");
    // unexpected extra arguments
    t.fold_same("x = 'abcdef'.includes({a: 2})");
    t.fold_same("x = 'abcdef'.includes([1, 2])");
    t.fold_same("x = tag `Hello ${name}`.includes('a')");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringStartsWith
#[test]
fn test_fold_string_starts_with() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold String.prototype.startsWith with Constant Arguments
    // Baseline current behavior and guards:
    // Under ECMA-262 § 22.1.3.24, String.prototype.startsWith evaluates substring prefix matching
    // with position clamping.
    // Future optimization fold targets:
    // - 'abcdef'.startsWith('abc') -> true
    // - 'abcdef'.startsWith('def') -> false
    // - 'abcdef'.startsWith('bc') -> false
    // - 'abcdef'.startsWith('bc', 1) -> true
    // - 'abcdef'.startsWith('bc', 2) -> false
    // - 'abcdef'.startsWith('abc', -5) -> true
    // - 'abcdef'.startsWith('', 2) -> true
    // - 'abcdef'.startsWith('') -> true
    // - ''.startsWith('') -> true
    // - 'abcdef'.startsWith('bc', 100) -> false
    // - '12345'.startsWith(1) -> true
    // - 'true'.startsWith(true) -> true
    // - '1abcdef'.startsWith(1) -> true
    // - 'abc1def'.startsWith(1) -> false
    // - 'trueabcdef'.startsWith(true) -> true
    // - 'abctruedef'.startsWith(true) -> false
    // - 'nullabcdef'.startsWith(null) -> true
    // - 'undefinedabcdef'.startsWith(undefined) -> true
    // - 'NaNabcdef'.startsWith(NaN) -> true
    t.fold_same("x = 'abcdef'.startsWith('abc')");
    t.fold_same("x = 'abcdef'.startsWith('def')");
    t.fold_same("x = 'abcdef'.startsWith('bc')");
    t.fold_same("x = 'abcdef'.startsWith('')");
    t.fold_same("x = ''.startsWith('')");
    // Positional search & clamping
    t.fold_same("x = 'abcdef'.startsWith('bc', 1)");
    t.fold_same("x = 'abcdef'.startsWith('bc', 2)");
    t.fold_same("x = 'abcdef'.startsWith('abc', -5)");
    t.fold_same("x = 'abcdef'.startsWith('', 2)");
    t.fold_same("x = 'abcdef'.startsWith('bc', 100)");
    // Coercions
    t.fold_same("x = '12345'.startsWith(1)");
    t.fold_same("x = 'true'.startsWith(true)");
    t.fold_same("x = '1abcdef'.startsWith(1)");
    t.fold_same("x = 'abc1def'.startsWith(1)");
    t.fold_same("x = 'trueabcdef'.startsWith(true)");
    t.fold_same("x = 'abctruedef'.startsWith(true)");
    t.fold_same("x = 'nullabcdef'.startsWith(null)");
    t.fold_same("x = 'undefinedabcdef'.startsWith(undefined)");
    t.fold_same("x = 'NaNabcdef'.startsWith(NaN)");
    // Negative / Guard cases (Must NOT fold)
    t.fold_same("x = str.startsWith('a')");
    // non-literal receiver
    t.fold_same("x = 'abc'.startsWith(y)");
    // non-constant argument
    t.fold_same("x = 'abc'.startsWith((foo(), 'a'))");
    // side-effecting argument
    t.fold_same("x = 'abc'.startsWith(/a/)");
    // regex argument throws TypeError at runtime (ECMA-262 §
    // 22.1.3.24)
    t.fold_same("x = 'abcdef'.startsWith(/abc/)");
    t.fold_same("x = 'abcdef'.startsWith('bc', pos)");
    // non-constant position
    t.fold_same("x = 'abcdef'.startsWith('bc', 1, 2)");
    // unexpected extra arguments
    t.fold_same("x = 'abcdef'.startsWith({a: 2})");
    t.fold_same("x = 'abcdef'.startsWith([1, 2])");
    t.fold_same("x = tag `Hello ${name}`.startsWith('a')");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringEndsWith
#[test]
fn test_fold_string_ends_with() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold String.prototype.endsWith with Constant Arguments
    // Baseline current behavior and guards:
    // Under ECMA-262 § 22.1.3.7, String.prototype.endsWith evaluates substring suffix matching with
    // endPosition clamping.
    // Future optimization fold targets:
    // - 'abcdef'.endsWith('def') -> true
    // - 'abcdef'.endsWith('abc') -> false
    // - 'abcdef'.endsWith('de') -> false
    // - 'abcdef'.endsWith('abc', 3) -> true
    // - 'abcdef'.endsWith('bcd', 4) -> true
    // - 'abcdef'.endsWith('de', 5) -> true
    // - 'abcdef'.endsWith('def', 100) -> true
    // - 'abcdef'.endsWith('', -5) -> true
    // - 'abcdef'.endsWith('a', -5) -> false
    // - 'abcdef'.endsWith('') -> true
    // - ''.endsWith('') -> true
    // - '12345'.endsWith(5) -> true
    // - 'true'.endsWith(true) -> true
    // - 'abcdef1'.endsWith(1) -> true
    // - '1abcdef'.endsWith(1) -> false
    // - 'abcdeftrue'.endsWith(true) -> true
    // - 'trueabcdef'.endsWith(true) -> false
    // - 'abcdefnull'.endsWith(null) -> true
    // - 'abcdefundefined'.endsWith(undefined) -> true
    // - 'abcdefNaN'.endsWith(NaN) -> true
    t.fold_same("x = 'abcdef'.endsWith('def')");
    t.fold_same("x = 'abcdef'.endsWith('abc')");
    t.fold_same("x = 'abcdef'.endsWith('de')");
    t.fold_same("x = 'abcdef'.endsWith('')");
    t.fold_same("x = ''.endsWith('')");
    // Positional search & clamping
    t.fold_same("x = 'abcdef'.endsWith('abc', 3)");
    t.fold_same("x = 'abcdef'.endsWith('bcd', 4)");
    t.fold_same("x = 'abcdef'.endsWith('de', 5)");
    t.fold_same("x = 'abcdef'.endsWith('def', 100)");
    t.fold_same("x = 'abcdef'.endsWith('', -5)");
    t.fold_same("x = 'abcdef'.endsWith('a', -5)");
    // Coercions
    t.fold_same("x = '12345'.endsWith(5)");
    t.fold_same("x = 'true'.endsWith(true)");
    t.fold_same("x = 'abcdef1'.endsWith(1)");
    t.fold_same("x = '1abcdef'.endsWith(1)");
    t.fold_same("x = 'abcdeftrue'.endsWith(true)");
    t.fold_same("x = 'trueabcdef'.endsWith(true)");
    t.fold_same("x = 'abcdefnull'.endsWith(null)");
    t.fold_same("x = 'abcdefundefined'.endsWith(undefined)");
    t.fold_same("x = 'abcdefNaN'.endsWith(NaN)");
    // Negative / Guard cases (Must NOT fold)
    t.fold_same("x = str.endsWith('a')");
    // non-literal receiver
    t.fold_same("x = 'abc'.endsWith(y)");
    // non-constant argument
    t.fold_same("x = 'abc'.endsWith((foo(), 'a'))");
    // side-effecting argument
    t.fold_same("x = 'abc'.endsWith(/def/)");
    // regex argument throws TypeError at runtime (ECMA-262 §
    // 22.1.3.7)
    t.fold_same("x = 'abcdef'.endsWith(/def/)");
    t.fold_same("x = 'abcdef'.endsWith('de', pos)");
    // non-constant endPosition
    t.fold_same("x = 'abcdef'.endsWith('de', 5, 2)");
    // unexpected extra arguments
    t.fold_same("x = 'abcdef'.endsWith({a: 2})");
    t.fold_same("x = 'abcdef'.endsWith([1, 2])");
    t.fold_same("x = tag `Hello ${name}`.endsWith('a')");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringTrimStart
#[test]
fn test_fold_string_trim_start() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold String.prototype.trimStart / trimLeft with Constant Arguments
    // Baseline current behavior and guards:
    // Under ECMA-262 § 22.1.3.34 (trimStart) and Annex § B.2.2.15 (trimLeft), leading WhiteSpace
    // and LineTerminator characters are removed.
    // Future optimization fold targets:
    // - '   foo   '.trimStart() -> 'foo   '
    // - '   foo   '.trimLeft() -> 'foo   '
    // - 'foo   '.trimStart() -> 'foo   '
    // - 'foo   '.trimLeft() -> 'foo   '
    // - ''.trimStart() -> ''
    // - ''.trimLeft() -> ''
    // - '   '.trimStart() -> ''
    // - '   '.trimLeft() -> ''
    // - '\\uFEFF\\u00A0\\t\\n foo \\t\\n'.trimStart() -> 'foo \\t\\n'
    // - '\\uFEFF\\u00A0\\t\\n foo \\t\\n'.trimLeft() -> 'foo \\t\\n'
    // - '\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000foo \\t'.trimStart() -> 'foo \\t'
    // - '\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000foo \\t'.trimLeft() -> 'foo \\t'
    t.fold_same("x = '   foo   '.trimStart()");
    t.fold_same("x = '   foo   '.trimLeft()");
    t.fold_same("x = 'foo   '.trimStart()");
    t.fold_same("x = 'foo   '.trimLeft()");
    t.fold_same("x = ''.trimStart()");
    t.fold_same("x = ''.trimLeft()");
    t.fold_same("x = '   '.trimStart()");
    t.fold_same("x = '   '.trimLeft()");
    t.fold_same("x = '\\uFEFF\\u00A0\\t\\n foo \\t\\n'.trimStart()");
    t.fold_same("x = '\\uFEFF\\u00A0\\t\\n foo \\t\\n'.trimLeft()");
    t.fold_same(
        "x = '\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000foo \\t'.trimStart()",
    );
    t.fold_same("x = '\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000foo \\t'.trimLeft()");
    // Negative / Guard cases (Must NOT fold)
    t.fold_same("x = str.trimStart()");
    // non-literal receiver
    t.fold_same("x = str.trimLeft()");
    t.fold_same("x = '   foo   '.trimStart(1)");
    // unexpected extra arguments
    t.fold_same("x = '   foo   '.trimLeft(1)");
    t.fold_same("x = '   foo   '.trimStart(foo())");
    // side-effecting argument
    t.fold_same("x = '   foo   '.trimLeft(foo())");
    t.fold_same("x = tag `   foo   `.trimStart()");
    t.fold_same("x = tag `   foo   `.trimLeft()");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringTrimEnd
#[test]
fn test_fold_string_trim_end() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold String.prototype.trimEnd / trimRight with Constant Arguments
    // Baseline current behavior and guards:
    // Under ECMA-262 § 22.1.3.33 (trimEnd) and Annex § B.2.2.16 (trimRight), trailing WhiteSpace
    // and LineTerminator characters are removed.
    // Future optimization fold targets:
    // - '   foo   '.trimEnd() -> '   foo'
    // - '   foo   '.trimRight() -> '   foo'
    // - '   foo'.trimEnd() -> '   foo'
    // - '   foo'.trimRight() -> '   foo'
    // - ''.trimEnd() -> ''
    // - ''.trimRight() -> ''
    // - '   '.trimEnd() -> ''
    // - '   '.trimRight() -> ''
    // - '\\t\\n foo \\uFEFF\\u00A0\\t\\n'.trimEnd() -> '\\t\\n foo'
    // - '\\t\\n foo \\uFEFF\\u00A0\\t\\n'.trimRight() -> '\\t\\n foo'
    // - '\\t foo\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000'.trimEnd() -> '\\t foo'
    // - '\\t foo\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000'.trimRight() -> '\\t foo'
    t.fold_same("x = '   foo   '.trimEnd()");
    t.fold_same("x = '   foo   '.trimRight()");
    t.fold_same("x = '   foo'.trimEnd()");
    t.fold_same("x = '   foo'.trimRight()");
    t.fold_same("x = ''.trimEnd()");
    t.fold_same("x = ''.trimRight()");
    t.fold_same("x = '   '.trimEnd()");
    t.fold_same("x = '   '.trimRight()");
    t.fold_same("x = '\\t\\n foo \\uFEFF\\u00A0\\t\\n'.trimEnd()");
    t.fold_same("x = '\\t\\n foo \\uFEFF\\u00A0\\t\\n'.trimRight()");
    t.fold_same("x = '\\t foo\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000'.trimEnd()");
    t.fold_same(
        "x = '\\t foo\\u1680\\u2000\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000'.trimRight()",
    );
    // Negative / Guard cases (Must NOT fold)
    t.fold_same("x = str.trimEnd()");
    // non-literal receiver
    t.fold_same("x = str.trimRight()");
    t.fold_same("x = '   foo   '.trimEnd(1)");
    // unexpected extra arguments
    t.fold_same("x = '   foo   '.trimRight(1)");
    t.fold_same("x = '   foo   '.trimEnd(foo())");
    // side-effecting argument
    t.fold_same("x = '   foo   '.trimRight(foo())");
    t.fold_same("x = tag `   foo   `.trimEnd()");
    t.fold_same("x = tag `   foo   `.trimRight()");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringAt
#[test]
fn test_fold_string_at() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold String.prototype.at with Constant Index
    // Baseline current behavior and guards:
    // Under ECMA-262 § 22.1.3.1, String.prototype.at evaluates relative indexing with negative
    // index normalization and out-of-bounds undefined (void 0) return.
    // Future optimization fold targets:
    // - 'hello'.at() -> 'h'
    // - 'hello'.at(undefined) -> 'h'
    // - 'hello'.at(0) -> 'h'
    // - 'hello'.at(1) -> 'e'
    // - 'hello'.at(4) -> 'o'
    // - 'hello'.at(-1) -> 'o'
    // - 'hello'.at(-2) -> 'l'
    // - 'hello'.at(-5) -> 'h'
    // - 'hello'.at(5) -> void 0
    // - 'hello'.at(10) -> void 0
    // - 'hello'.at(-6) -> void 0
    // - 'hello'.at(-10) -> void 0
    // - ''.at(0) -> void 0
    // - ''.at(-1) -> void 0
    // - 'hello'.at(Infinity) -> void 0
    // - 'hello'.at(-Infinity) -> void 0
    // - 'hello'.at(1.9) -> 'e'
    // - 'hello'.at(-1.9) -> 'o'
    // - 'hello'.at(0.5) -> 'h'
    // - 'hello'.at(-0.5) -> 'h'
    // - 'hello'.at(4.1) -> 'o'
    // - 'hello'.at(4.9) -> 'o'
    // - '123'.at(0) -> '1'
    // - 'hello'.at(null) -> 'h'
    // - 'hello'.at(false) -> 'h'
    // - 'hello'.at(true) -> 'e'
    // - 'hello'.at(NaN) -> 'h'
    // - 'hello'.at('1') -> 'e'
    // - '\\ud834\udd1e'.at(0) -> '\\ud834'
    // - '\\ud834\udd1e'.at(1) -> '\\udd1e'
    // - '\\ud834\udd1e'.at(-1) -> '\\udd1e'
    // - '\\ud834\udd1e'.at(-2) -> '\\ud834'
    t.fold_same("x = 'hello'.at()");
    t.fold_same("x = 'hello'.at(undefined)");
    t.fold_same("x = 'hello'.at(0)");
    t.fold_same("x = 'hello'.at(1)");
    t.fold_same("x = 'hello'.at(4)");
    // Negative relative indices
    t.fold_same("x = 'hello'.at(-1)");
    t.fold_same("x = 'hello'.at(-2)");
    t.fold_same("x = 'hello'.at(-5)");
    // Out-of-bounds (evaluates to void 0)
    t.fold_same("x = 'hello'.at(5)");
    t.fold_same("x = 'hello'.at(10)");
    t.fold_same("x = 'hello'.at(-6)");
    t.fold_same("x = 'hello'.at(-10)");
    t.fold_same("x = ''.at(0)");
    t.fold_same("x = ''.at(-1)");
    t.fold_same("x = 'hello'.at(Infinity)");
    t.fold_same("x = 'hello'.at(-Infinity)");
    // Floating-point truncation
    t.fold_same("x = 'hello'.at(1.9)");
    t.fold_same("x = 'hello'.at(-1.9)");
    t.fold_same("x = 'hello'.at(0.5)");
    t.fold_same("x = 'hello'.at(-0.5)");
    t.fold_same("x = 'hello'.at(4.1)");
    t.fold_same("x = 'hello'.at(4.9)");
    t.fold_same("x = 'hello'.at(-5.1)");
    t.fold_same("x = 'hello'.at(-5.9)");
    t.fold_same("x = 'a'.at(-1.5)");
    t.fold_same("x = 'hello'.at(-6.0)");
    t.fold_same("x = 'hello'.at(-6.1)");
    // Coercions
    t.fold_same("x = '123'.at(0)");
    t.fold_same("x = 'hello'.at(null)");
    t.fold_same("x = 'hello'.at(false)");
    t.fold_same("x = 'hello'.at(true)");
    t.fold_same("x = 'hello'.at(NaN)");
    t.fold_same("x = 'hello'.at('1')");
    // Surrogate pairs & code units
    t.fold_same(with_unit("x = '\\ud834", 0xdd1e, "'.at(0)"));
    t.fold_same(with_unit("x = '\\ud834", 0xdd1e, "'.at(1)"));
    t.fold_same(with_unit("x = '\\ud834", 0xdd1e, "'.at(-1)"));
    t.fold_same(with_unit("x = '\\ud834", 0xdd1e, "'.at(-2)"));
    // Negative / Guard cases (Must NOT fold)
    t.fold_same("x = str.at(0)");
    // non-literal receiver
    t.fold_same("x = 'hello'.at(y)");
    // non-constant argument
    t.fold_same("x = 'hello'.at((foo(), 1))");
    // side-effecting argument
    t.fold_same("x = 'hello'.at(foo())");
    t.fold_same("x = 'hello'.at(0, 1)");
    // unexpected extra arguments
    t.fold_same("x = 'hello'.at(0, foo())");
    t.fold_same("x = 'hello'.at({a: 1})");
    t.fold_same("x = 'hello'.at([1])");
    t.fold_same("x = `hello`.at(0)");
    t.fold_same("x = `hello ${name}`.at(0)");
    t.fold_same("x = tag `hello`.at(0)");
}

// port: PeepholeReplaceKnownMethodsTest#testStringJoinAddSparse
#[test]
fn test_string_join_add_sparse() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = [,,'a'].join(',')", "x = ',,a'");
}

// port: PeepholeReplaceKnownMethodsTest#testNoStringJoin
#[test]
fn test_no_string_join() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("x = [].join(',',2)");
    t.fold_same("x = [].join(f)");
}

// port: PeepholeReplaceKnownMethodsTest#testStringJoinAdd
#[test]
fn test_string_join_add() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = ['a', 'b', 'c'].join('')", "x = \"abc\"");
    t.fold("x = [].join(',')", "x = \"\"");
    t.fold("x = ['a'].join(',')", "x = \"a\"");
    t.fold("x = ['a', 'b', 'c'].join(',')", "x = \"a,b,c\"");
    t.fold(
        "x = ['a', foo, 'b', 'c'].join(',')",
        "x = [\"a\",foo,\"b,c\"].join()",
    );
    t.fold(
        "x = [foo, 'a', 'b', 'c'].join(',')",
        "x = [foo,\"a,b,c\"].join()",
    );
    t.fold(
        "x = ['a', 'b', 'c', foo].join(',')",
        "x = [\"a,b,c\",foo].join()",
    );
    // Works with numbers
    t.fold("x = ['a=', 5].join('')", "x = \"a=5\"");
    t.fold("x = ['a', '5'].join(7)", "x = \"a75\"");
    // Works on boolean
    t.fold("x = ['a=', false].join('')", "x = \"a=false\"");
    t.fold("x = ['a', '5'].join(true)", "x = \"atrue5\"");
    t.fold("x = ['a', '5'].join(false)", "x = \"afalse5\"");
    // Only optimize if it's a size win.
    t.fold(
        "x = ['a', '5', 'c'].join('a very very very long chain')",
        "x = [\"a\",\"5\",\"c\"].join(\"a very very very long chain\")",
    );
    // Template strings
    t.fold("x = [`a`, `b`, `c`].join(``)", "x = 'abc'");
    t.fold("x = [`a`, `b`, `c`].join('')", "x = 'abc'");
    // TODO(user): Its possible to fold this better.
    t.fold_same("x = ['', foo].join('-')");
    t.fold_same("x = ['', foo, ''].join()");
    t.fold(
        "x = ['', '', foo, ''].join(',')",
        "x = [ ','  , foo, ''].join()",
    );
    t.fold(
        "x = ['', '', foo, '', ''].join(',')",
        "x = [ ',',   foo,  ','].join()",
    );
    t.fold(
        "x = ['', '', foo, '', '', bar].join(',')",
        "x = [ ',',   foo,  ',',   bar].join()",
    );
    t.fold("x = [1,2,3].join('abcdef')", "x = '1abcdef2abcdef3'");
    t.fold("x = [1,2].join()", "x = '1,2'");
    t.fold("x = [1, 2].join(undefined)", "x = '1,2'");
    t.fold("x = [1, 2].join(void 0)", "x = '1,2'");
    t.fold("x = [null,undefined,''].join(',')", "x = ',,'");
    t.fold("x = [null,undefined,0].join(',')", "x = ',,0'");
    // This can be folded but we don't currently.
    t.fold_same("x = [[1,2],[3,4]].join()");
    // would like: "x = '1,2,3,4'"
}

// port: PeepholeReplaceKnownMethodsTest#testStringJoinAdd_b1992789
#[test]
fn test_string_join_add_b1992789() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = ['a'].join('')", "x = \"a\"");
    t.fold_same("x = [foo()].join('')");
    t.fold_same("[foo()].join('')");
    t.fold("[null].join('')", "''");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringSubstr
#[test]
fn test_fold_string_substr() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcde'.substr(0,2)", "x = 'ab'");
    t.fold("x = 'abcde'.substr(1,2)", "x = 'bc'");
    t.fold("x = 'abcde'.substr(2)", "x = 'cde'");
    // we should be leaving negative indexes alone for now
    t.fold_same("x = 'abcde'.substr(-1)");
    t.fold_same("x = 'abcde'.substr(1, -2)");
    t.fold_same("x = 'abcde'.substr(1, 2, 3)");
    t.fold_same("x = 'a'.substr(0, 2)");
    // Template strings
    t.fold_same("x = `abcdef`.substr(0,2)");
    t.fold_same("x = `abc ${xyz} def`.substr(0,2)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringReplace
#[test]
fn test_fold_string_replace() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("'c'.replace('c','x')", "'x'");
    t.fold("'ac'.replace('c','x')", "'ax'");
    t.fold("'ca'.replace('c','x')", "'xa'");
    t.fold("'ac'.replace('c','xxx')", "'axxx'");
    t.fold("'ca'.replace('c','xxx')", "'xxxa'");
    // only one instance replaced
    t.fold("'acaca'.replace('c','x')", "'axaca'");
    t.fold("'ab'.replace('','x')", "'xab'");
    t.fold_same("'acaca'.replace(/c/,'x')");
    // this will affect the global RegExp props
    t.fold_same("'acaca'.replace(/c/g,'x')");
    // this will affect the global RegExp props
    // not a literal
    t.fold_same("x.replace('x','c')");
    t.fold_same("'Xyz'.replace('Xyz', '$$')");
    // would fold to '$'
    t.fold_same("'PreXyzPost'.replace('Xyz', '$&')");
    // would fold to 'PreXyzPost'
    t.fold_same("'PreXyzPost'.replace('Xyz', '$`')");
    // would fold to 'PrePrePost'
    t.fold_same("'PreXyzPost'.replace('Xyz', '$\\'')");
    // would fold to  'PrePostPost'
    t.fold_same("'PreXyzPostXyz'.replace('Xyz', '$\\'')");
    // would fold to 'PrePostXyzPostXyz'
    t.fold_same("'123'.replace('2', '$`')");
    // would fold to '113'
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringReplaceAll
#[test]
fn test_fold_string_replace_all() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcde'.replaceAll('bcd','c')", "x = 'ace'");
    t.fold("x = 'abcde'.replaceAll('c','xxx')", "x = 'abxxxde'");
    t.fold("x = 'abcde'.replaceAll('xxx','c')", "x = 'abcde'");
    t.fold("'ab'.replaceAll('','x')", "'xaxbx'");
    t.fold("x = 'c_c_c'.replaceAll('c','x')", "x = 'x_x_x'");
    t.fold_same("x = 'acaca'.replaceAll(/c/,'x')");
    // this should throw
    t.fold_same("x = 'acaca'.replaceAll(/c/g,'x')");
    // this will affect the global RegExp props
    // not a literal
    t.fold_same("x.replaceAll('x','c')");
    t.fold_same("'Xyz'.replaceAll('Xyz', '$$')");
    // would fold to '$'
    t.fold_same("'PreXyzPost'.replaceAll('Xyz', '$&')");
    // would fold to 'PreXyzPost'
    t.fold_same("'PreXyzPost'.replaceAll('Xyz', '$`')");
    // would fold to 'PrePrePost'
    t.fold_same("'PreXyzPost'.replaceAll('Xyz', '$\\'')");
    // would fold to  'PrePostPost'
    t.fold_same("'PreXyzPostXyz'.replaceAll('Xyz', '$\\'')");
    // would fold to 'PrePostXyzPost'
    t.fold_same("'123'.replaceAll('2', '$`')");
    // would fold to '113'
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringSubstring
#[test]
fn test_fold_string_substring() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcde'.substring(0,2)", "x = 'ab'");
    t.fold("x = 'abcde'.substring(1,2)", "x = 'b'");
    t.fold("x = 'abcde'.substring(2)", "x = 'cde'");
    // we should be leaving negative, out-of-bound, and inverted indices alone for now
    t.fold_same("x = 'abcde'.substring(-1)");
    t.fold_same("x = 'abcde'.substring(1, -2)");
    t.fold_same("x = 'abcde'.substring(1, 2, 3)");
    t.fold_same("x = 'abcde'.substring(2, 0)");
    t.fold_same("x = 'a'.substring(0, 2)");
    // Template strings
    t.fold_same("x = `abcdef`.substring(0,2)");
    t.fold_same("x = `abcdef ${abc}`.substring(0,2)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringSlice
#[test]
fn test_fold_string_slice() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcde'.slice(0,2)", "x = 'ab'");
    t.fold("x = 'abcde'.slice(1,2)", "x = 'b'");
    t.fold("x = 'abcde'.slice(2)", "x = 'cde'");
    // we should be leaving negative, out-of-bound, and inverted indices alone for now
    t.fold_same("x = 'abcde'.slice(-1)");
    t.fold_same("x = 'abcde'.slice(1, -2)");
    t.fold_same("x = 'abcde'.slice(1, 2, 3)");
    t.fold_same("x = 'abcde'.slice(2, 0)");
    t.fold_same("x = 'a'.slice(0, 2)");
    // Template strings
    t.fold_same("x = `abcdef`.slice(0,2)");
    t.fold_same("x = `abcdef ${abc}`.slice(0,2)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringCharAt
#[test]
fn test_fold_string_char_at() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcde'.charAt(0)", "x = 'a'");
    t.fold("x = 'abcde'.charAt(1)", "x = 'b'");
    t.fold("x = 'abcde'.charAt(2)", "x = 'c'");
    t.fold("x = 'abcde'.charAt(3)", "x = 'd'");
    t.fold("x = 'abcde'.charAt(4)", "x = 'e'");
    t.fold_same("x = 'abcde'.charAt(5)");
    // or x = ''
    t.fold_same("x = 'abcde'.charAt(-1)");
    // or x = ''
    t.fold_same("x = 'abcde'.charAt(y)");
    t.fold_same("x = 'abcde'.charAt()");
    // or x = 'a'
    t.fold_same("x = 'abcde'.charAt(0, ++z)");
    // or (++z, 'a')
    t.fold_same("x = 'abcde'.charAt(null)");
    // or x = 'a'
    t.fold_same("x = 'abcde'.charAt(true)");
    // or x = 'b'
    t.fold(
        with_unit("x = '\\ud834", 0xdd1e, "'.charAt(0)"),
        "x = '\\ud834'",
    );
    t.fold(
        with_unit("x = '\\ud834", 0xdd1e, "'.charAt(1)"),
        "x = '\\udd1e'",
    );
    // Template strings
    t.fold_same("x = `abcdef`.charAt(0)");
    t.fold_same("x = `abcdef ${abc}`.charAt(0)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringCharCodeAt
#[test]
fn test_fold_string_char_code_at() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = 'abcde'.charCodeAt(0)", "x = 97");
    t.fold("x = 'abcde'.charCodeAt(1)", "x = 98");
    t.fold("x = 'abcde'.charCodeAt(2)", "x = 99");
    t.fold("x = 'abcde'.charCodeAt(3)", "x = 100");
    t.fold("x = 'abcde'.charCodeAt(4)", "x = 101");
    t.fold_same("x = 'abcde'.charCodeAt(5)");
    // or x = (0/0)
    t.fold_same("x = 'abcde'.charCodeAt(-1)");
    // or x = (0/0)
    t.fold_same("x = 'abcde'.charCodeAt(y)");
    t.fold_same("x = 'abcde'.charCodeAt()");
    // or x = 97
    t.fold_same("x = 'abcde'.charCodeAt(0, ++z)");
    // or (++z, 97)
    t.fold_same("x = 'abcde'.charCodeAt(null)");
    // or x = 97
    t.fold_same("x = 'abcde'.charCodeAt(true)");
    // or x = 98
    t.fold(
        with_unit("x = '\\ud834", 0xdd1e, "'.charCodeAt(0)"),
        "x = 55348",
    );
    t.fold(
        with_unit("x = '\\ud834", 0xdd1e, "'.charCodeAt(1)"),
        "x = 56606",
    );
    // Template strings
    t.fold_same("x = `abcdef`.charCodeAt(0)");
    t.fold_same("x = `abcdef ${abc}`.charCodeAt(0)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldStringSplit
#[test]
fn test_fold_string_split() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.hooks.late = false;
    t.fold("x = 'abcde'.split('foo')", "x = ['abcde']");
    t.fold("x = 'abcde'.split()", "x = ['abcde']");
    t.fold("x = 'abcde'.split(null)", "x = ['abcde']");
    t.fold("x = 'a b c d e'.split(' ')", "x = ['a','b','c','d','e']");
    t.fold("x = 'a b c d e'.split(' ', 0)", "x = []");
    t.fold("x = 'abcde'.split('cd')", "x = ['ab','e']");
    t.fold("x = 'a b c d e'.split(' ', 1)", "x = ['a']");
    t.fold("x = 'a b c d e'.split(' ', 3)", "x = ['a','b','c']");
    t.fold("x = 'a b c d e'.split(null, 1)", "x = ['a b c d e']");
    t.fold("x = 'aaaaa'.split('a')", "x = ['', '', '', '', '', '']");
    t.fold("x = 'xyx'.split('x')", "x = ['', 'y', '']");
    // Empty separator
    t.fold("x = 'abcde'.split('')", "x = ['a','b','c','d','e']");
    t.fold("x = 'abcde'.split('', 3)", "x = ['a','b','c']");
    // Empty separator AND empty string
    t.fold("x = ''.split('')", "x = []");
    // Separator equals string
    t.fold("x = 'aaa'.split('aaa')", "x = ['','']");
    t.fold("x = ' '.split(' ')", "x = ['','']");
    t.fold_same("x = 'abcde'.split(/ /)");
    t.fold_same("x = 'abcde'.split(' ', -1)");
    // Template strings
    t.fold_same("x = `abcdef`.split()");
    t.fold_same("x = `abcdef ${abc}`.split()");
    t.hooks.late = true;
    t.fold_same("x = 'a b c d e'.split(' ')");
}

// port: PeepholeReplaceKnownMethodsTest#testJoinBug
#[test]
fn test_join_bug() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("var x = [].join();", "var x = '';");
    t.fold_same("var x = [x].join();");
    t.fold_same("var x = [x,y].join();");
    t.fold_same("var x = [x,y,z].join();");
    t.fold_same("shape['matrix'] = [\n    Number(headingCos2).toFixed(4),\n    Number(-headingSin2).toFixed(4),\n    Number(headingSin2 * yScale).toFixed(4),\n    Number(headingCos2 * yScale).toFixed(4),\n    0,\n    0\n  ].join()\n");
}

// port: PeepholeReplaceKnownMethodsTest#testJoinSpread1
#[test]
fn test_join_spread1() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("var x = [...foo].join('');");
    t.fold_same("var x = [...someMap.keys()].join('');");
    t.fold_same("var x = [foo, ...bar].join('');");
    t.fold_same("var x = [...foo, bar].join('');");
    t.fold_same("var x = [...foo, 'bar'].join('');");
    t.fold_same("var x = ['1', ...'2', '3'].join('');");
    t.fold_same("var x = ['1', ...['2'], '3'].join('');");
}

// port: PeepholeReplaceKnownMethodsTest#testJoinSpread2
#[test]
fn test_join_spread2() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("var x = [...foo].join(',');", "var x = [...foo].join();");
    t.fold(
        "var x = [...someMap.keys()].join(',');",
        "var x = [...someMap.keys()].join();",
    );
    t.fold(
        "var x = [foo, ...bar].join(',');",
        "var x = [foo, ...bar].join();",
    );
    t.fold(
        "var x = [...foo, bar].join(',');",
        "var x = [...foo, bar].join();",
    );
    t.fold(
        "var x = [...foo, 'bar'].join(',');",
        "var x = [...foo, 'bar'].join();",
    );
    t.fold(
        "var x = ['1', ...'2', '3'].join(',');",
        "var x = ['1', ...'2', '3'].join();",
    );
    t.fold(
        "var x = ['1', ...['2'], '3'].join(',');",
        "var x = ['1', ...['2'], '3'].join();",
    );
}

// port: PeepholeReplaceKnownMethodsTest#testToUpper
#[test]
fn test_to_upper() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("'a'.toUpperCase()", "'A'");
    t.fold("'A'.toUpperCase()", "'A'");
    t.fold("'aBcDe'.toUpperCase()", "'ABCDE'");
    t.fold_same("`abc`.toUpperCase()");
    t.fold_same("`a ${bc}`.toUpperCase()");
    // Make sure things aren't totally broken for non-ASCII strings, non-exhaustive.
    // <p>This includes things like:
    // <ul>
    // <li>graphemes with multiple code-points
    // <li>graphemes represented by multiple graphemes in other cases
    // <li>graphemes whose case changes are not round-trippable
    // <li>graphemes that change case in a position sentitive way
    // </ul>
    // /
    t.fold("'I'.toUpperCase()", "'I'");
    t.fold("'i'.toUpperCase()", "'I'");
    t.fold("'\u{130}'.toUpperCase()", "'\u{130}'");
    t.fold("'\u{131}'.toUpperCase()", "'I'");
    t.fold("'I\u{307}'.toUpperCase()", "'I\u{307}'");
    t.fold("'\u{df}'.toUpperCase()", "'SS'");
    t.fold("'SS'.toUpperCase()", "'SS'");
    t.fold("'\u{3c3}'.toUpperCase()", "'\u{3a3}'");
    t.fold("'\u{3c3}\u{3c2}'.toUpperCase()", "'\u{3a3}\u{3a3}'");
}

// port: PeepholeReplaceKnownMethodsTest#testToLower
#[test]
fn test_to_lower() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("'A'.toLowerCase()", "'a'");
    t.fold("'a'.toLowerCase()", "'a'");
    t.fold("'aBcDe'.toLowerCase()", "'abcde'");
    t.fold_same("`ABC`.toLowerCase()");
    t.fold_same("`A ${BC}`.toLowerCase()");
    // Make sure things aren't totally broken for non-ASCII strings, non-exhaustive.
    // <p>This includes things like:
    // <ul>
    // <li>graphemes with multiple code-points
    // <li>graphemes with multiple representations
    // <li>graphemes represented by multiple graphemes in other cases
    // <li>graphemes whose case changes are not round-trippable
    // <li>graphemes that change case in a position sentitive way
    // </ul>
    // /
    t.fold("'I'.toLowerCase()", "'i'");
    t.fold("'i'.toLowerCase()", "'i'");
    t.fold("'\u{130}'.toLowerCase()", "'i\u{307}'");
    t.fold("'\u{131}'.toLowerCase()", "'\u{131}'");
    t.fold("'I\u{307}'.toLowerCase()", "'i\u{307}'");
    t.fold("'\u{df}'.toLowerCase()", "'\u{df}'");
    t.fold("'SS'.toLowerCase()", "'ss'");
    t.fold("'\u{3a3}'.toLowerCase()", "'\u{3c3}'");
    t.fold("'\u{3a3}\u{3a3}'.toLowerCase()", "'\u{3c3}\u{3c2}'");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctionsBug
#[test]
fn test_fold_math_functions_bug() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math[0]()");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_abs
#[test]
fn test_fold_math_functions_abs() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.abs(Math.random())");
    t.fold("Math.abs('-1')", "1");
    t.fold("Math.abs(-2)", "2");
    t.fold("Math.abs(null)", "0");
    t.fold("Math.abs('')", "0");
    t.fold("Math.abs([])", "0");
    t.fold("Math.abs([2])", "2");
    t.fold("Math.abs([1,2])", "NaN");
    t.fold("Math.abs({})", "NaN");
    t.fold("Math.abs('string');", "NaN");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_imul
#[test]
fn test_fold_math_functions_imul() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.imul(Math.random(),2)");
    t.fold("Math.imul(-1,1)", "-1");
    t.fold("Math.imul(2,2)", "4");
    t.fold("Math.imul(2)", "0");
    t.fold("Math.imul(2,3,5)", "6");
    t.fold("Math.imul(0xfffffffe, 5)", "-10");
    t.fold("Math.imul(0xffffffff, 5)", "-5");
    t.fold(
        "Math.imul(0xfffffffffffff34f, 0xfffffffffff342)",
        "13369344",
    );
    t.fold(
        "Math.imul(0xfffffffffffff34f, -0xfffffffffff342)",
        "-13369344",
    );
    t.fold("Math.imul(NaN, 2)", "0");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_ceil
#[test]
fn test_fold_math_functions_ceil() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.ceil(Math.random())");
    t.fold("Math.ceil(1)", "1");
    t.fold("Math.ceil(1.5)", "2");
    t.fold("Math.ceil(1.3)", "2");
    t.fold("Math.ceil(-1.3)", "-1");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_floor
#[test]
fn test_fold_math_functions_floor() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.floor(Math.random())");
    t.fold("Math.floor(1)", "1");
    t.fold("Math.floor(1.5)", "1");
    t.fold("Math.floor(1.3)", "1");
    t.fold("Math.floor(-1.3)", "-2");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_fround
#[test]
fn test_fold_math_functions_fround() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.fround(Math.random())");
    t.fold("Math.fround(NaN)", "NaN");
    t.fold("Math.fround(Infinity)", "Infinity");
    t.fold("Math.fround(1)", "1");
    t.fold("Math.fround(0)", "0");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_fround_j2cl
#[test]
fn test_fold_math_functions_fround_j2cl() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.fround(1.2)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_round
#[test]
fn test_fold_math_functions_round() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.round(Math.random())");
    t.fold("Math.round(NaN)", "NaN");
    t.fold("Math.round(3.5)", "4");
    t.fold("Math.round(-3.5)", "-3");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_sign
#[test]
fn test_fold_math_functions_sign() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.sign(Math.random())");
    t.fold("Math.sign(NaN)", "NaN");
    t.fold("Math.sign(3.5)", "1");
    t.fold("Math.sign(-3.5)", "-1");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_trunc
#[test]
fn test_fold_math_functions_trunc() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.trunc(Math.random())");
    t.fold("Math.sign(NaN)", "NaN");
    t.fold("Math.trunc(3.5)", "3");
    t.fold("Math.trunc(-3.5)", "-3");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_clz32
#[test]
fn test_fold_math_functions_clz32() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("Math.clz32(0)", "32");
    let mut x: i32 = 1;
    let mut i = 31;
    while i >= 0 {
        t.fold(format!("Math.clz32({x})"), format!("{i}"));
        t.fold(
            format!("Math.clz32({})", 2i32.wrapping_mul(x).wrapping_sub(1)),
            format!("{i}"),
        );
        x = x.wrapping_mul(2);
        i -= 1;
    }
    t.fold("Math.clz32('52')", "26");
    t.fold("Math.clz32([52])", "26");
    t.fold("Math.clz32([52, 53])", "32");
    // Overflow cases
    t.fold("Math.clz32(0x100000000)", "32");
    t.fold("Math.clz32(0x100000001)", "31");
    // NaN -> 0
    t.fold("Math.clz32(NaN)", "32");
    t.fold("Math.clz32('foo')", "32");
    t.fold("Math.clz32(Infinity)", "32");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_max
#[test]
fn test_fold_math_functions_max() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.max(Math.random(), 1)");
    t.fold("Math.max()", "-Infinity");
    t.fold("Math.max(0)", "0");
    t.fold("Math.max(0, 1)", "1");
    t.fold("Math.max(0, 1, -1, 200)", "200");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_min
#[test]
fn test_fold_math_functions_min() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Math.min(Math.random(), 1)");
    t.fold("Math.min()", "Infinity");
    t.fold("Math.min(3)", "3");
    t.fold("Math.min(0, 1)", "0");
    t.fold("Math.min(0, 1, -1, 200)", "-1");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldMathFunctions_pow
#[test]
fn test_fold_math_functions_pow() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("Math.pow(1, 2)", "1");
    t.fold("Math.pow(2, 0)", "1");
    t.fold("Math.pow(2, 2)", "4");
    t.fold("Math.pow(2, 32)", "4294967296");
    t.fold("Math.pow(Infinity, 0)", "1");
    t.fold("Math.pow(Infinity, 1)", "Infinity");
    t.fold("Math.pow('a', 33)", "NaN");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldNumberFunctions_isSafeInteger
#[test]
fn test_fold_number_functions_is_safe_integer() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("Number.isSafeInteger(1)", "true");
    t.fold("Number.isSafeInteger(1.5)", "false");
    t.fold("Number.isSafeInteger(9007199254740991)", "true");
    t.fold("Number.isSafeInteger(9007199254740992)", "false");
    t.fold("Number.isSafeInteger(-9007199254740991)", "true");
    t.fold("Number.isSafeInteger(-9007199254740992)", "false");
    t.fold("Number.isSafeInteger(undefined)", "false");
    t.fold("Number.isSafeInteger('str')", "false");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldNumberFunctions_isFinite
#[test]
fn test_fold_number_functions_is_finite() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("Number.isFinite(1)", "true");
    t.fold("Number.isFinite(1.5)", "true");
    t.fold("Number.isFinite(NaN)", "false");
    t.fold("Number.isFinite(Infinity)", "false");
    t.fold("Number.isFinite(-Infinity)", "false");
    t.fold("Number.isFinite(undefined)", "false");
    t.fold("Number.isFinite(null)", "false");
    t.fold("Number.isFinite('str')", "false");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldNumberFunctions_isNaN
#[test]
fn test_fold_number_functions_is_na_n() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("Number.isNaN(1)", "false");
    t.fold("Number.isNaN(1.5)", "false");
    t.fold("Number.isNaN(NaN)", "true");
    t.fold("Number.isNaN(undefined)", "false");
    t.fold("Number.isNaN(void 0)", "false");
    t.fold("Number.isNaN(null)", "false");
    t.fold("Number.isNaN('str')", "false");
    t.fold("Number.isNaN(0)", "false");
    // unknown function may have side effects
    t.fold_same("Number.isNaN(+(void unknown()))");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldParseNumbers
#[test]
fn test_fold_parse_numbers() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Template Strings
    t.fold_same("x = parseInt(`123`)");
    t.fold_same("x = parseInt(` 123`)");
    t.fold_same("x = parseInt(`12 ${a}`)");
    t.fold_same("x = parseFloat(`1.23`)");
    t.harness
        .set_accepted_language(language_mode("ECMASCRIPT5"))
        .unwrap();
    t.fold("x = parseInt('123')", "x = 123");
    t.fold("x = parseInt(' 123')", "x = 123");
    t.fold("x = parseInt('123', 10)", "x = 123");
    t.fold("x = parseInt('0xA')", "x = 10");
    t.fold("x = parseInt('0xA', 16)", "x = 10");
    t.fold("x = parseInt('07', 8)", "x = 7");
    t.fold("x = parseInt('08')", "x = 8");
    t.fold("x = parseInt('0')", "x = 0");
    t.fold("x = parseInt('-0')", "x = -0");
    t.fold("x = parseFloat('0')", "x = 0");
    t.fold("x = parseFloat('1.23')", "x = 1.23");
    t.fold("x = parseFloat('-1.23')", "x = -1.23");
    t.fold("x = parseFloat('1.2300')", "x = 1.23");
    t.fold("x = parseFloat(' 0.3333')", "x = 0.3333");
    t.fold("x = parseFloat('0100')", "x = 100");
    t.fold("x = parseFloat('0100.000')", "x = 100");
    // Mozilla Dev Center test cases
    t.fold("x = parseInt(' 0xF', 16)", "x = 15");
    t.fold("x = parseInt(' F', 16)", "x = 15");
    t.fold("x = parseInt('17', 8)", "x = 15");
    t.fold("x = parseInt('015', 10)", "x = 15");
    t.fold("x = parseInt('1111', 2)", "x = 15");
    t.fold("x = parseInt('12', 13)", "x = 15");
    t.fold("x = parseInt(15.99, 10)", "x = 15");
    t.fold("x = parseInt(-15.99, 10)", "x = -15");
    t.fold("x = parseInt('-15.99', 10)", "x = -15");
    t.fold("x = parseFloat('3.14')", "x = 3.14");
    t.fold("x = parseFloat(3.14)", "x = 3.14");
    t.fold("x = parseFloat(-3.14)", "x = -3.14");
    t.fold("x = parseFloat('-3.14')", "x = -3.14");
    t.fold("x = parseFloat('-0')", "x = -0");
    // Valid calls - trailing non-digits or hex prefixes
    t.fold("x = parseInt('FXX123', 16)", "x = 15");
    t.fold("x = parseInt('15*3', 10)", "x = 15");
    t.fold("x = parseInt('15e2', 10)", "x = 15");
    t.fold("x = parseInt('15px', 10)", "x = 15");
    t.fold("x = parseInt('-0x08')", "x = -8");
    t.fold("x = parseInt('0xa', 10)", "x = 0");
    t.fold("x = parseInt('+123')", "x = 123");
    t.fold("x = parseInt('+0xA')", "x = 10");
    t.fold_same("x = parseInt('1', -1)");
    t.fold_same("x = parseFloat('3.14more non-digit characters')");
    t.fold_same("x = parseFloat('314e-2')");
    t.fold_same("x = parseFloat('0.0314E+2')");
    t.fold_same("x = parseFloat('3.333333333333333333333333')");
    // Invalid calls / un-foldable
    t.fold_same("x = parseInt('')");
    // Large numbers and precision tests (beyond 32-bit int)
    t.fold("x = parseInt('2147483648')", "x = 2147483648");
    t.fold("x = parseInt(2147483648)", "x = 2147483648");
    t.fold("x = parseInt('9007199254740991')", "x = 9007199254740991");
    t.fold("x = parseInt(9007199254740991)", "x = 9007199254740991");
    t.fold("x = parseInt('0x80000000')", "x = 2147483648");
    t.fold("x = parseInt('0x80000000', 16)", "x = 2147483648");
    t.fold("x = parseInt(1234567890123.45)", "x = 1234567890123");
    t.fold("x = parseInt(1e21)", "x = 1");
    t.fold("x = parseInt(0.0000001)", "x = 1");
    t.harness
        .set_accepted_language(language_mode("ECMASCRIPT3"))
        .unwrap();
    t.fold_same("x = parseInt('08')");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldParseOctalNumbers
#[test]
fn test_fold_parse_octal_numbers() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.harness
        .set_accepted_language(language_mode("ECMASCRIPT5"))
        .unwrap();
    t.fold("x = parseInt('021', 8)", "x = 17");
    t.fold("x = parseInt('-021', 8)", "x = -17");
}

// port: PeepholeReplaceKnownMethodsTest#testReplaceWithCharAt
#[test]
fn test_replace_with_char_at() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.harness.enable_type_check().unwrap();
    t.harness.replace_types_with_colors().unwrap();
    t.harness.disable_compare_js_doc().unwrap();
    t.fold_string_typed("a.substring(0, 1)", "a.charAt(0)");
    t.fold_same_string_typed("a.substring(-4, -3)");
    t.fold_same_string_typed("a.substring(i, j + 1)");
    t.fold_same_string_typed("a.substring(i, i + 1)");
    t.fold_same_string_typed("a.substring(1, 2, 3)");
    t.fold_same_string_typed("a.substring()");
    t.fold_same_string_typed("a.substring(1)");
    t.fold_same_string_typed("a.substring(1, 3, 4)");
    t.fold_same_string_typed("a.substring(-1, 3)");
    t.fold_same_string_typed("a.substring(2, 1)");
    t.fold_same_string_typed("a.substring(3, 1)");
    t.fold_string_typed("a.slice(4, 5)", "a.charAt(4)");
    t.fold_same_string_typed("a.slice(-2, -1)");
    t.fold_string_typed(
        "var /** number */ i; a.slice(0, 1)",
        "var /** number */ i; a.charAt(0)",
    );
    t.fold_same_string_typed("a.slice(i, j + 1)");
    t.fold_same_string_typed("a.slice(i, i + 1)");
    t.fold_same_string_typed("a.slice(1, 2, 3)");
    t.fold_same_string_typed("a.slice()");
    t.fold_same_string_typed("a.slice(1)");
    t.fold_same_string_typed("a.slice(1, 3, 4)");
    t.fold_same_string_typed("a.slice(-1, 3)");
    t.fold_same_string_typed("a.slice(2, 1)");
    t.fold_same_string_typed("a.slice(3, 1)");
    t.fold_string_typed("a.substr(0, 1)", "a.charAt(0)");
    t.fold_string_typed("a.substr(2, 1)", "a.charAt(2)");
    t.fold_same_string_typed("a.substr(-2, 1)");
    t.fold_same_string_typed("a.substr(bar(), 1)");
    t.fold_same_string_typed("''.substr(bar(), 1)");
    t.fold_same_string_typed("a.substr(2, 1, 3)");
    t.fold_same_string_typed("a.substr(1, 2, 3)");
    t.fold_same_string_typed("a.substr()");
    t.fold_same_string_typed("a.substr(1)");
    t.fold_same_string_typed("a.substr(1, 2)");
    t.fold_same_string_typed("a.substr(1, 2, 3)");
    t.harness.enable_type_check().unwrap();
    t.fold_same("function f(/** ? */ a) { a.substring(0, 1); }");
    t.fold_same("function f(/** ? */ a) { a.substr(0, 1); }");
    t.fold_same("/** @constructor */ function A() {};\nA.prototype.substring = function(begin$jscomp$1, end$jscomp$1) {};\nfunction f(/** !A */ a) { a.substring(0, 1); }\n");
    t.fold_same("/** @constructor */ function A() {};\nA.prototype.slice = function(begin$jscomp$1, end$jscomp$1) {};\nfunction f(/** !A */ a) { a.slice(0, 1); }\n");
    t.hooks.use_types = false;
    t.fold_same_string_typed("a.substring(0, 1)");
    t.fold_same_string_typed("a.substr(0, 1)");
    t.fold_same_string_typed("''.substring(i, i + 1)");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldConcatChaining
#[test]
fn test_fold_concat_chaining() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.harness.enable_type_check().unwrap();
    t.fold(
        "[1,2].concat(1).concat(2,['abc']).concat('abc')",
        "[1,2].concat(1,2,['abc'],'abc')",
    );
    t.fold(
        "[].concat(['abc']).concat(1).concat([2,3])",
        "['abc'].concat(1,[2,3])",
    );
    // cannot fold concat based on type information
    t.fold_same("returnArrayType().concat(returnArrayType()).concat(1).concat(2)");
    t.fold_same("returnArrayType().concat(returnUnionType()).concat(1).concat(2)");
    t.fold(
        "[1,2,1].concat(1).concat(returnArrayType()).concat(2)",
        "[1,2,1].concat(1).concat(returnArrayType(),2)",
    );
    t.fold(
        "[1].concat(1).concat(2).concat(returnArrayType())",
        "[1].concat(1,2).concat(returnArrayType())",
    );
    t.fold_same("[].concat(1).concat(returnArrayType())");
    t.fold_same("obj.concat([1,2]).concat(1)");
}

// port: PeepholeReplaceKnownMethodsTest#testRemoveArrayLiteralFromFrontOfConcat
#[test]
fn test_remove_array_literal_from_front_of_concat() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.harness.enable_type_check().unwrap();
    t.fold("[].concat([1,2,3],1)", "[1,2,3].concat(1)");
    t.fold_same("[1,2,3].concat(returnArrayType())");
    // Call method with the same name as Array.prototype.concat
    t.fold_same("obj.concat([1,2,3])");
    t.fold_same("[].concat(1,[1,2,3])");
    t.fold_same("[].concat(1)");
    t.fold("[].concat([1])", "[1].concat()");
    // Chained folding of empty array lit
    t.fold("[].concat([], [1,2,3], [4])", "[1,2,3].concat([4])");
    t.fold(
        "[].concat([]).concat([1]).concat([2,3])",
        "[1].concat([2,3])",
    );
    // Cannot fold based on type information
    t.fold_same("[].concat(returnArrayType(),1)");
    t.fold_same("[].concat(returnArrayType())");
    t.fold_same("[].concat(returnUnionType())");
}

// port: PeepholeReplaceKnownMethodsTest#testArrayOfSpread
#[test]
fn test_array_of_spread() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold(
        "x = Array.of(...['a', 'b', 'c'])",
        "x = [...['a', 'b', 'c']]",
    );
    t.fold(
        "x = Array.of(...['a', 'b', 'c',])",
        "x = [...['a', 'b', 'c']]",
    );
    t.fold(
        "x = Array.of(...['a'], ...['b', 'c'])",
        "x = [...['a'], ...['b', 'c']]",
    );
    t.fold(
        "x = Array.of('a', ...['b', 'c'])",
        "x = ['a', ...['b', 'c']]",
    );
    t.fold(
        "x = Array.of('a', ...['b', 'c'])",
        "x = ['a', ...['b', 'c']]",
    );
}

// port: PeepholeReplaceKnownMethodsTest#testArrayOfNoSpread
#[test]
fn test_array_of_no_spread() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = Array.of('a', 'b', 'c')", "x = ['a', 'b', 'c']");
    t.fold("x = Array.of('a', ['b', 'c'])", "x = ['a', ['b', 'c']]");
    t.fold("x = Array.of('a', ['b', 'c'],)", "x = ['a', ['b', 'c']]");
}

// port: PeepholeReplaceKnownMethodsTest#testArrayOfNoArgs
#[test]
fn test_array_of_no_args() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold("x = Array.of()", "x = []");
}

// port: PeepholeReplaceKnownMethodsTest#testArrayOfNoChange
#[test]
fn test_array_of_no_change() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("x = Array.of.apply(window, ['a', 'b', 'c'])");
    t.fold_same("x = ['a', 'b', 'c']");
    t.fold_same("x = [Array.of, 'a', 'b', 'c']");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldArrayBug
#[test]
fn test_fold_array_bug() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    t.fold_same("Array[123]()");
}

// port: PeepholeReplaceKnownMethodsTest#testFoldArrayIsArray
#[test]
fn test_fold_array_is_array() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // Fold Array.isArray with Constant and Literal Arguments
    // Baseline current behavior and guards:
    // Under ECMA-262 § 23.1.2.2 and § 7.2.2 (IsArray), Array.isArray determines whether the
    // argument is an Array exotic object.
    // Future optimization fold targets:
    // - Array literals:
    //   - Array.isArray([]) -> true
    //   - Array.isArray([1, 2, 3]) -> true
    //   - Array.isArray(['a', 'b']) -> true
    // - Non-array primitives:
    //   - Array.isArray(123) -> false
    //   - Array.isArray(0) -> false
    //   - Array.isArray('hello') -> false
    //   - Array.isArray('') -> false
    //   - Array.isArray(true) -> false
    //   - Array.isArray(false) -> false
    //   - Array.isArray(null) -> false
    //   - Array.isArray(undefined) -> false
    //   - Array.isArray(void 0) -> false
    //   - Array.isArray(NaN) -> false
    //   - Array.isArray(Infinity) -> false
    //   - Array.isArray(-Infinity) -> false
    // - Object literals & other reference types:
    //   - Array.isArray({}) -> false
    //   - Array.isArray({0: 'a', length: 1}) -> false
    //   - Array.isArray(/abc/) -> false
    //   - Array.isArray(function() {}) -> false
    //   - Array.isArray(() => {}) -> false
    // - Omitted argument:
    //   - Array.isArray() -> false (arg evaluates to undefined)
    // Positive fold cases (Array literals)
    t.fold_same("x = Array.isArray([])");
    t.fold_same("x = Array.isArray([1, 2, 3])");
    t.fold_same("x = Array.isArray(['a', 'b'])");
    // Positive fold cases (Non-array primitives)
    t.fold_same("x = Array.isArray(123)");
    t.fold_same("x = Array.isArray(0)");
    t.fold_same("x = Array.isArray('hello')");
    t.fold_same("x = Array.isArray('')");
    t.fold_same("x = Array.isArray(true)");
    t.fold_same("x = Array.isArray(false)");
    t.fold_same("x = Array.isArray(null)");
    t.fold_same("x = Array.isArray(undefined)");
    t.fold_same("x = Array.isArray(void 0)");
    t.fold_same("x = Array.isArray(NaN)");
    t.fold_same("x = Array.isArray(Infinity)");
    t.fold_same("x = Array.isArray(-Infinity)");
    // Positive fold cases (Object literals & other reference types)
    t.fold_same("x = Array.isArray({})");
    t.fold_same("x = Array.isArray({0: 'a', length: 1})");
    t.fold_same("x = Array.isArray(/abc/)");
    t.fold_same("x = Array.isArray(function() {})");
    t.fold_same("x = Array.isArray(() => {})");
    // Positive fold cases (Omitted argument -> evaluates as undefined)
    t.fold_same("x = Array.isArray()");
    // Negative / Guard cases (MUST NOT fold)
    t.fold_same("x = Array.isArray(x)");
    // unknown variable
    t.fold_same("x = Array.isArray(foo())");
    // side-effecting function call
    t.fold_same("x = Array.isArray((foo(), []))");
    // side-effecting sequence expression
    t.fold_same("x = Array.isArray([foo()])");
    // array literal containing side-effecting element
    t.fold_same("x = Array.isArray([...x])");
    // array literal with spread element
    t.fold_same("x = Array.isArray([], 1)");
    // unexpected extra arguments
    t.fold_same("x = window.Array.isArray([])");
    // non-standard qualified receiver
    t.fold(
        "function f(Array) { return Array.isArray([]); }",
        "function f(Array$jscomp$1) { return Array$jscomp$1.isArray([]); }",
    );
    // shadowed Array
    // identifier
}

// port: PeepholeReplaceKnownMethodsTest#testBatchD_objectStaticMethods
#[test]
fn test_batch_d_object_static_methods() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // OPP-016: Object.keys, Object.values, Object.entries guards and current behavior
    t.fold_same("x = Object.keys({a: 1, b: 2})");
    t.fold_same("x = Object.keys({})");
    t.fold_same("x = Object.values({a: 1, b: 2})");
    t.fold_same("x = Object.entries({a: 1, b: 2})");
    t.fold_same("x = Object.keys(obj)");
    t.fold_same("x = Object.keys({a: foo(), b: 2})");
    t.fold_same("x = Object.values({get a() { return 1; }})");
    // OPP-017: Object.assign guards and current behavior
    t.fold_same("x = Object.assign({}, {a: 1}, {b: 2})");
    t.fold_same("x = Object.assign({a: 1}, {b: 2})");
    t.fold_same("x = Object.assign(target, {})");
    t.fold_same("x = Object.assign({}, obj)");
    t.fold_same("x = Object.assign({}, {a: foo()})");
    t.fold_same("x = Object.assign({}, {get a() { return 1; }})");
    // OPP-018: Object.is guards and current behavior
    t.fold_same("x = Object.is('a', 'a')");
    t.fold_same("x = Object.is(1, 1)");
    t.fold_same("x = Object.is(NaN, NaN)");
    t.fold_same("x = Object.is(0, -0)");
    t.fold_same("x = Object.is(a, 'hello')");
    t.fold_same("x = Object.is(a, b)");
}

// port: PeepholeReplaceKnownMethodsTest#testBatchD_mathAndNumberStaticMethods
#[test]
fn test_batch_d_math_and_number_static_methods() {
    let mut t = PeepholeReplaceKnownMethodsTest::new();
    // OPP-019: Number.isInteger, Number.isFinite, Number.isNaN, Math methods
    t.fold_same("x = Number.isInteger(1)");
    t.fold_same("x = Number.isInteger(1.5)");
    t.fold_same("x = Number.isInteger('1')");
    t.fold_same("x = Number.isInteger(NaN)");
    t.fold_same("x = Number.isInteger(Infinity)");
    t.fold_same("x = Number.isInteger(x)");
    // Existing Math/Number fold checks and edge guards
    t.fold("Math.abs(-5)", "5");
    t.fold("Math.abs(5)", "5");
    t.fold("Math.sign(5)", "1");
    t.fold("Math.sign(-5)", "-1");
    t.fold("Math.trunc(5.7)", "5");
    t.fold("Math.trunc(-5.7)", "-5");
    t.fold("Math.clz32(1)", "31");
    t.fold("Number.isFinite(100)", "true");
    t.fold("Number.isFinite(Infinity)", "false");
    t.fold("Number.isNaN(NaN)", "true");
    t.fold("Number.isNaN(100)", "false");
    t.fold("Number.isNaN('hello')", "false");
    t.fold("Number.isFinite('100')", "false");
    t.fold_same("Number.isFinite(x)");
    t.fold_same("Math.abs(x)");
    t.fold_same("Math.sign(x)");
    t.fold_same("Math.trunc(x)");
}
