# Processor descriptor DSL (v2)

Implemented by `oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java` (expressions),
`ReplayCompilerTest.java`, `ReplayTypeCheckTest.java`, `integration/ReplayIntegrationTest.java`
and `ReplayOptions.java` (case keys). A Rust replay harness must implement the same semantics;
every construct below is fully specified. v2 only adds constructs: every v1 descriptor means the
same thing in v2, except for the stricter overload rule (see "Overload resolution").

## Descriptor file
`corpus/unit/descriptors/<TestClassSimpleName>.json`:

```
{"source": "<where getProcessor lives>",
 "classMap": {"<recorded FQCN>": "<helper FQCN>", ...},      (optional)
 "cases": [{"when": {...}, <case keys>}, ...]}
```

Other top-level keys (for example `unrepresentable`, `pendingCases`, `note`) and other case keys
(for example `note`) are notes for humans and are ignored by replay. Gate 0.2 reads these
top-level keys: `unrepresentable` (`[{index, reason}]`, records excluded from (a) and counted in
(e)), `testFieldsAfterUnchecked` (see the case key), and `postCallAssertions`
(`[{"methods": [names] | "*", "status": "out_of_scope", "reason": "..."}]`, FORMAT.md
"Post-call state"; superseded by corpus/unit/postcall/, listed only), and `vacuityNoopClass`
(an FQCN or a list of FQCNs: the src pass the harness itself runs, mutated with the class-level
NoopAgent in the vacuity run; FORMAT.md "No-op vacuity, harness-run passes"). `vacuityAudit` is not
read: a free-text reason cannot keep a vacuous class green; the mechanical
rule in FORMAT.md "Post-call state" applies instead.

**Case selection:** the first case whose `when` matches is used. `when` maps a dotted path into the
record (for example `testFields.withNormalize`) to a JSON value. A path segment that is a decimal
number indexes a JSON array. A path matches when the record's JSON at that path equals the value
exactly (JSON equality; a missing path equals only `null`). `{"isNull": true|false}` instead tests
for (non-)null or missing. `{}` always matches.
- `compiler_test_case` records: a matching case is required (no match is a harness error).
- `integration` and `type_check` records: cases are optional. If none matches, the record is
  replayed with no customisation. Such a descriptor may still have `"cases": []`.

### Case keys
| Key | Kinds | Meaning |
|---|---|---|
| `processor`: Expr | compiler_test_case | evaluated once per `getProcessor(compiler)` call, so once per repetition (`harness.effective.getNumRepetitions`); must yield a `CompilerPass` |
| `compilerSetup`: [Expr] | compiler_test_case, type_check | effects run, in order, on each freshly created compiler with `{"compiler":true}` bound to it. compiler_test_case: inside `createCompiler()`, right after the harness's own `createCompiler()` and before `init`/parse (models a test's `createCompiler` override that returns a plain Compiler after side-effecting calls, e.g. `J2clSourceFileChecker.markToRunJ2clPasses`, `forwardDeclareType`). type_check: after `initOptions(options)` and `markFeatureNotAllowed(MODULES)`, before the `TypeTestBuilder` is built (models a test that changes the instance `compiler` before `newTest...().run()`). |
| `options`: `{"skip":[names], "then":[Expr]}` | all | while the recorded options are rebuilt (FORMAT.md "Replay"), the top-level option field names in `skip` are not decoded from the record; afterwards each `then` Expr is evaluated with `{"options":true}` bound to the options being built. No compiler is in scope. Use it for option values the record cannot carry (for example an anonymous class from the test). |
| `postconditions`: [Expr] | compiler_test_case, api `testInternal` | each Expr must yield a `CompilerTestCase.Postcondition`; they are passed to `testInternal` in order, so they run exactly where the original postconditions ran. Typically a verbatim helper rebuilt from `{"record":"expected.postconditionValues.0.captures.arg$2"}` (FORMAT.md). |
| `testFieldsAfter`: Expr | compiler_test_case | post-call check. After the call, for every expected key `k` (FORMAT.md "Post-call state": the record's `testFieldsAfter` keys, plus `testFieldsAfterAlso`, minus `testFieldsAfterSkip`), field `k` of the object the Expr yields (typically the `once` helper holder the processor used) is dumped with the FORMAT.md value encoding (depth 2), helper class names are mapped back through `classMap`, and the result must equal the expected values exactly. |
| `testFieldsAfterAlso`: [name] | compiler_test_case | fields checked even when the call left them unchanged; expected value = `testFieldsAfter[name]` if present, else `testFields[name]`. |
| `testFieldsAfterSkip`: {name: reason} | compiler_test_case | fields left out of the check; every reason must be a non-empty string (harness error otherwise). |
| `testFieldsAfterUnchecked`: reason | compiler_test_case (case or descriptor top level) | declares that the post-call assertions of the records this case selects cannot be represented; they become **notCaptured** (never green in gate (a), counted in gate (e)). |

**classMap:** every class lookup made while replaying this descriptor's records (decoding
`object`/`classRef`/`enum`/`arrayOf`/options `@class`, and the DSL's `new`/`static`/`enum`/
`array`/`class`/`lambda` names) first replaces a name found in `classMap` by its value. Use it to
map a class declared inside a `*Test` class (which rule 6 keeps off the replay classpath), such as
`FooTest$1` or `FooTest$Guard`, to a verbatim helper class. A recorded `{"object":...}` value is
then rebuilt on the helper class from its recorded fields (FORMAT.md "Value encoding").

## Evaluation order
Every expression is evaluated at most once per evaluation of its parent. Arguments (`args`),
`list`/`set`/`array` items, `map` entries (key, then value), `let` bindings and `do` effects are
evaluated left to right, exactly once each, before the call or construction they belong to.
`withFields` values are evaluated in JSON key order. A `lambda` body is evaluated each time the
lambda is invoked, not when the lambda is created.

## Expressions
Each expression is a JSON object with exactly one tag key, plus that tag's own keys.

| Expr | Value |
|---|---|
| `{"int":1}` | 32-bit int |
| `{"long":"1"}` | 64-bit int (decimal string) |
| `{"double":"1.5"}` | IEEE double (Java `Double.parseDouble`) |
| `{"bool":true}` / `{"string":"s"}` / `{"null":true}` | boolean / string / null |
| `{"enum":"FQCN","name":"X"}` | enum constant |
| `{"class":"FQCN"}` | the class object (a Java class literal, e.g. for `TextFormat.parse(text, Cls.class)`) |
| `{"list":[E]}` / `{"set":[E]}` / `{"map":[[E,E]]}` | insertion-ordered list / set / map |
| `{"array":"FQCN","items":[E]}` | Java array with that component type (also used for varargs) |
| `{"compiler":true}` | the Compiler in scope: the one passed to `getProcessor`, or the one being set up in `compilerSetup` |
| `{"options":true}` | the options being built inside an `options.then` list; otherwise `compiler.getOptions()` |
| `{"field":"name"}` | the recorded `testFields.name`, decoded with the FORMAT.md value encoding. If absent, the first key that ends in `.name` (a field inherited from a test superclass). |
| `{"field":"name","path":"a.b"}` | the raw JSON at `testFields.name.a.b`, decoded |
| `{"record":"dotted.path"}` | any decoded record value, for example `harness.effective.getName` or `expected.postconditionValues.0.captures.arg$1` (numeric segments index arrays) |
| `{"new":"FQCN","args":[E]}` | constructor call (non-public constructors allowed) |
| `{"static":"FQCN","method":"m","args":[E]}` | static method call |
| `{"call":E,"method":"m","args":[E]}` | instance method call. A builder chain is nested `call`s. |
| `{"withFields":E,"fields":{"f":E}}` | evaluates E, sets its fields reflectively, returns it |
| `{"getField":E,"name":"f"}` | reads field `f` of E's value reflectively (FORMAT.md field naming: own field, or `Declaring.f` for an inherited one) |
| `{"once":"key","value":E}` | evaluates `value` once per record and reuses that object in every later evaluation with the same key in the same record (every `getProcessor` call, `postconditions`, `testFieldsAfter`). Models a test-instance field shared across repetitions, such as a stateful id supplier or the test instance itself. |
| `{"let":[["x",E],...],"in":E}` | binds each name in order (a later binding sees the earlier ones), then evaluates `in` with them in scope |
| `{"var":"x"}` | the value bound to `x` by the innermost enclosing `let` or `lambda` parameter; unbound is an error |
| `{"do":[E...],"value":E}` | evaluates the effects in order, then returns `value` (for void calls such as `addOneTimePass`) |
| `{"lambda":["p",...],"iface":"FQCN","body":E}` | an instance of the functional interface `iface`. Calling its single abstract method binds the arguments to the parameter names, on top of the variables visible where the lambda was created, and returns the value of `body`, evaluated at call time. The parameter count must equal the method's arity. A void method discards the value. Default methods and `Object` methods keep their usual behaviour (`equals` is identity). |
| `{"sequence":[E]}` | a CompilerPass running each pass in order on (externs, root); the passes are built when the sequence is evaluated |
| `{"helper":"Holder.Inner","outer":["f"],"args":[E]}` | instantiates helper class `Inner`, nested in `<package>.Holder` (see below) |
| `{"mutationPoint":"Name","value":E}` | evaluates E; under `--mutate-noop Name` returns a no-op stand-in with the role of E's value instead (see "No-op mutation"), otherwise E's value |

Patterns that need no dedicated construct:
- **A pass built at process time** (a getProcessor lambda that constructs passes inside
  `process`): `{"lambda":["externs","root"],"iface":"com.google.javascript.jscomp.CompilerPass","body":{"do":[{"call":{"new":"...A","args":[{"compiler":true}]},"method":"process","args":[{"var":"externs"},{"var":"root"}]}, ...],"value":{"null":true}}}`.
- **A traversal pass** `(externs, root) -> NodeTraversal.traverse(compiler, root, cb)`: the same
  lambda with body `{"static":"com.google.javascript.jscomp.NodeTraversal","method":"traverse","args":[{"compiler":true},{"var":"root"},CB]}` (or `traverseRoots` with `CB, externs, root`).
- **A pass factory** `makePassFactory("n", Ctor::new)`: `{"static":"com.google.javascript.jscomp.CompilerTestCase","method":"makePassFactory","args":[{"string":"n"},{"lambda":["c"],"iface":"java.util.function.Function","body":{"new":"...Ctor","args":[{"var":"c"}]}}]}`.
- **A PhaseOptimizer**: `{"let":[["opt",{"new":"com.google.javascript.jscomp.PhaseOptimizer","args":[{"compiler":true},{"null":true}]}]],"in":{"do":[{"call":{"var":"opt"},"method":"addOneTimePass","args":[F]}],"value":{"var":"opt"}}}`.

**Overload resolution** (`new`, `static`, `call`, `helper`): candidates are the constructors, or
the methods with that name and static-ness found on the class, its superclasses and the
interfaces of each, excluding compiler-generated bridge methods. Keep candidates with the right
arity. A candidate is applicable when each argument is null (for a reference parameter) or an
instance of the boxed parameter type. In addition, a list or set argument is accepted for a
parameter that `ImmutableList` or `ImmutableSet` can be assigned to, and a map argument for a
parameter that `ImmutableMap` can be assigned to; the argument is then copied into that type.
First try without widening; only if nothing is applicable, also let an int argument widen to
long or double. Among the applicable candidates choose the most specific one: a candidate whose
every parameter type (boxed) is assignable to the corresponding parameter type of every other
applicable candidate. Candidates with identical parameter types (an override and the method it
overrides) count once, and the most derived one is invoked. If no single most specific candidate
exists, replay fails with an "ambiguous overload" harness error; the descriptor must then pass
an argument of a more specific type. (v1 said "first in declaration order", which reflection
does not provide.)

## Helpers
`oracle/replay/helpers/<package path>/<Holder>.java` holds code copied **verbatim** from a test
class, inside a generated holder class. The header names the source file, the line ranges and the
commit (bb8c8e7). Helpers are part of the corpus that Rust ports.
- `"package"` (optional, default `com.google.javascript.jscomp`) is the holder's package, for
  helpers that need package-private access elsewhere, e.g.
  `{"helper":"JSTypeColorIdHasherTest_Helpers","package":"com.google.javascript.jscomp.serialization"}`.
- `{"helper":"Holder"}` with no `.Inner` constructs the holder itself with `args`.
- A static nested helper is constructed with `args`.
- A non-static inner helper needs an outer (holder) instance. With `"outerInstance": E`, E's
  value is used (it must be a `Holder`; E is evaluated after `args`); this lets the processor, `postconditions` and
  `testFieldsAfter` share one holder through `{"once":...}`. Otherwise a fresh holder instance is
  made (its no-argument constructor if it has one, otherwise no constructor runs), and each name in `outer` is a holder field (declared with
  the test's own name and type) that is set from `{"field":name}` first.

## No-op mutation (gate 0.2(d))
**Gate 0.2(d) uses (D-015 d) the class-level mutation of HARNESS.md
"Class-level no-op mutation" (a JVM agent; `scripts/unit_replay.sh --mutate-noop` uses it too).
The descriptor-level stand-in below remains available as `ReplayMain --mutate-noop` for
debugging; `mutationPoint` is no longer needed to reach passes built inside helpers.**

With `--mutate-noop X` (a simple or fully qualified class name), passes of class X are replaced
by a no-op stand-in. A class is *mutated* when its name, its simple name, or the name or simple
name of a class that encloses it equals X (so `X$Builder` and `X$Inner` count). A class has a
*pass role* when it is an `AbstractPeepholeOptimization`, an `AbstractPeepholeTranspilation`, a
`CompilerPass`, a `NodeTraversal.Callback` (or `ScopedCallback`) or an
`OptimizeCalls.CallGraphCompilerPass`.
- `new C(...)`: arguments are evaluated; if C is mutated and has a pass role, the constructor is
  not called and the value is the stand-in.
- `static` and `call`: if the result's runtime class is mutated and has a pass role, the value is
  the stand-in (so factories and builder `build()` calls are reached).
- `mutationPoint`: as in the table above (reaches passes built inside helper code).
- A `call` on a stand-in returns the stand-in itself without calling anything (its arguments are
  still evaluated); `withFields` on a stand-in sets nothing and returns it. So a builder chain
  whose head was replaced still yields the stand-in.

The stand-in has the same role(s) as the replaced class: an identity peephole optimization
(`optimizeSubtree` returns its argument), an identity peephole transpilation (no features
transpiled away, `transpileSubtree` returns its argument), or an object implementing each of
`CompilerPass`, `NodeTraversal.Callback`, `NodeTraversal.ScopedCallback` and
`OptimizeCalls.CallGraphCompilerPass` that the replaced class implements, where `shouldTraverse`
returns true and every other method does nothing.

## Whole-processor no-op (gate 0.2(a) vacuity run)
`ReplayMain --noop-processor` makes every `compiler_test_case` processor a pass that does
nothing, without evaluating the case's `processor` Expr. Everything else (post-call checks,
postconditions, diagnostics, output comparison) runs as usual. When that run catches nothing and the
descriptor names `vacuityNoopClass`, the gate replays the class again with the class-level NoopAgent
for each named pass (HARNESS.md "Class-level no-op mutation"); the processor is then built normally.

## Replay outcome
See FORMAT.md "Replay". `compiler_test_case` records with api `testInternal` or
`testExternChanges` both use the selected case; `testExternChanges` calls
`CompilerTestCase.testExternChanges(externs, sources, expected, diagnostics...)` and observes
errors and warnings on the compiler that its `createCompiler()` call returned.

## Constructs deliberately left out of the DSL
- `lazySequence`, `passFactory`, `ctorRef`, `traverse`, `traverseRoots` as separate tags: each is
  an instance of `lambda` + `let`/`do` (see "Patterns"), so the DSL stays small.
- Per-case `harness` / `getOptionsHarnessFields` overrides: fixed in the recorder instead.
  Records now carry `harness.fieldsAfterGetOptions` (FORMAT.md) and replay applies it every time
  `getOptions()` runs, for every class, with no descriptor work.
- Descriptor-level `createCompiler` and `sharedCompiler`: `compilerSetup` (case level, use
  `"when":{}`) covers the first; the second is unobservable in the records that asked for it.
- `observed.externExport` and similar per-pass observations: postcondition lambdas are now
  replayable through `expected.postconditionValues` + `postconditions`, which checks the test's
  own assertion instead of a new per-pass field.
- A `{"holderField":...}` tag: `{"getField":{"once":...},"name":...}` and `testFieldsAfter`
  cover it generically.

**Postconditions count rule.** A case used for a record whose
`expected.postconditions` is N must list exactly N `postconditions` Exprs. Otherwise the record is
`notCaptured` (FORMAT.md "Postconditions count rule"): excluded from gate (a), counted in gate (e).
