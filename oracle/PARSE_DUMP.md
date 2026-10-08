# `parse_dump` schema: the parser contract

`parse_dump` parses one file along the path the compiler itself uses:

```
CompilerInput(sourceFile, isExtern).getAstRoot(compiler)
  -> CompilerInput.JsAst.parse
  -> ParserRunner.parse(sourceFile, code, compiler.getParserConfig(DEFAULT | EXTERNS), errorReporter)
  -> IRFactory
```

It then emits canonical JSON. The Rust parser (`crates/parsing`, bin `parse_dump`) produces this
JSON byte for byte, after canonical re-serialisation.

## How the options are built

**Options.** `CompilerOptions` come from `CommandLineRunner.createOptions()` followed by
`AbstractCommandLineRunner.setRunOptions()` (the two steps `doRun` performs before parsing)
for the argv `["--language_in=<language_in>", ...args]`. So `--jscomp_error/--jscomp_warning/
--jscomp_off`, `--hide_warnings_for`, `--print_tree` and `--parse_inline_source_maps` apply.
Flags for which `setRunOptions` needs a `Compiler` (`--json_warnings_file`,
`--error_format=JSON`, `--skip_normal_outputs`) are rejected (`ok:false`). The optional
`jsdoc_parsing` field overrides `setParseJsDocDocumentation`.

**Threads.** The parse runs inside `Compiler.runInCompilerThread` (CompilerExecutor's
`jscompiler` thread, 64 MB stack), as in a real compile, so `JSC_PARSE_TREE_TOO_DEEP` appears
at the same depths. The dump and its JSON serialization run on a thread with a 1 GB stack;
there is no AST depth limit (5000-level ASTs are in `smoke.sh`). The JSON nests 2 levels per
AST level; Python's `json` handles about 20000 levels.

**The compiler.** The oracle runs `Compiler.init([], [], options)`, so
`getParserConfig()` is computed as during a real compile:
- the language mode;
- `StrictMode`, from `expectStrictModeInput()`;
- JSDoc parsing mode (the CLI default is `TYPES_ONLY`; `--print_tree` switches it to
  `INCLUDE_ALL_COMMENTS`);
- `RunMode` (`STOP_AFTER_ERROR` unless `--continue_after_errors` is given);
- extra annotation names;
- `parseInlineSourceMaps`.

**Externs.** With `"kind":"extern"`, the file is parsed with the EXTERNS config. That config
is ES5 when `language_in` is ES3.

**What is not done.** `CompilerInput.getAstRoot` itself sets `INPUT_ID` on the root and
records `FEATURE_SET`. Nothing else runs: no `parseInputs` hoisting, no externs merging, and no
`Es6RewriteModules` or other passes.

**Failed parses.** When the parse fails fatally, the root is the empty `SCRIPT` created by
`IR.script()`. That matches `JsAst.parse`.

## Top-level object

```json
{
  "schema": "closure-rs/parse_dump/2",
  "source_name": "path or name",
  "language_in": "ECMASCRIPT_2020",
  "parser_config": "<Config.toString()>",
  "errors":   [Diag...],
  "warnings": [Diag...],
  "features": ["FEATURE_NAME", ...],
  "ast": Node,
  "ok": true
}
```

- **`parser_config`:** `Config.toString()` of the parser config that was used. It is
  informational, so the Rust parser should reproduce its *fields*, not its string.
- **`errors` and `warnings`:** `compiler.getErrors()` and `compiler.getWarnings()`, in report
  order. Each element is
  `Diag = {"key": DiagnosticType.key, "level": "ERROR|WARNING", "description": text, "source_name", "lineno", "charno"}`.
- **`features`:** the names in `CompilerInput.getFeatures()`, sorted alphabetically.

## Node

```json
{
  "token": "NAME",                 // Token enum name
  "node_class": "StringNode",      // Node | StringNode | NumberNode | BigIntNode | TemplateLiteralSubstringNode
  "lineno": 1, "charno": 4,        // Node.getLineno()/getCharno()
  "source_offset": 4,              // Node.getSourceOffset(); derived, see below
  "source_position": 4100,         // Node.getSourcePosition() (packed lineno<<12 | charno, raw int)
  "length": 1,                     // Node.getLength()
  "source_name": "a.js",           // Node.getSourceFileName()
  ...value fields by node_class...,
  "props": { "PROP_NAME": value, ... },   // every Node.Prop present on the node, in Prop ordinal order
  "children": [Node, ...]
}
```

**Value fields:**
- **`StringNode`:** `"string"`, a JsString (see below).
- **`NumberNode`:** three fields.
  - `"double_bits"` is `"0x%016x"` of `Double.doubleToRawLongBits`. This is the exact value.
  - `"double_java"` is `Double.toString`.
  - `"double_closure"` is `new CodePrinter.Builder(numberNode).build()`, which is how
    Closure's code printer emits the literal (`CodeConsumer.addNumber`). It is null if the
    printer rejects the node.
- **`BigIntNode`:** `"bigint"`, the decimal `BigInteger.toString()`.
- **`TemplateLiteralSubstringNode`:** `"cooked"`, a JsString or null for an invalid escape in a
  tagged template, and `"raw"`, a JsString.

**JsString.** A JSON string when the value is well-formed UTF-16. When it contains any lone
surrogate it is instead `{"utf16":[code units...]}`, so no information is lost through
UTF-8 JSON.

**Derived positions.** `source_offset` and `source_position` are derived from
`SourceFile.getLineOffset`, which counts only `'\n'`. When the parser counted other line
terminators (CR, U+2028, U+2029, also inside string literals) the call throws; the field is then
`{"$threw": "<exception class>"}` instead of failing the dump. `lineno`/`charno` are the
parser's own values and are always present.

## Diagnostics and parser side outputs (schema 2)

- **`reports`**: every `JSError` reported during the parse, in report order, **before**
  warnings guards apply: `{key, level (default), effective_level, description, source_name,
  lineno, charno}`. `effective_level` is the level after the compiler's guards (`OFF` if
  suppressed). `errors`/`warnings` remain the post-guard error-manager lists.
- **`source_map_url`**: `ParseResult.sourceMapURL`, i.e. the **last** `//# sourceMappingURL=`
  comment, trimmed (last one wins), or null. `CompilerInput.JsAst.parse` uses it to register
  input source maps.
- **`comments`** (`[{type, value, location:{start,end:{line,column,offset}}}]`) and
  **`top_level_statement_ranges`**: `ParseResult.comments` / `topLevelStatementRanges`,
  populated only when the JSDoc mode parses descriptions (e.g. `--print_tree`,
  `INCLUDE_ALL_COMMENTS`).
- These three come from a second `ParserRunner.parse` of the same file with the same config
  and a silent error reporter (its diagnostics are discarded).

## Properties

The set of properties is exhaustive:
- `OracleNodeAccess` iterates `Node.Prop.values()`, which covers all 55 constants at the
  pinned commit, from `IS_PARENTHESIZED` to `TRAILING_COMMA`.
- For each constant it calls the package-private `Node.lookupProperty(prop)`.
- It emits every property that is present, whether it is an `IntPropListItem` or an
  `ObjectPropListItem`.

**Int properties** are emitted as the raw int, for example `"IS_SHORTHAND_PROPERTY": 1`,
`"SIDE_EFFECT_FLAGS": n` or `"INCRDECR": 1`.

**Object properties** are serialised according to the value's type:

| Value type | JSON |
|---|---|
| `String` (`NON_JSDOC_COMMENT`, …) | JsString |
| `JSDocInfo` | JSDocInfo object (below) |
| `Node` (`DECLARED_TYPE_EXPR`, `GENERIC_TYPE`, …) | Node |
| `JSTypeExpression` | `{"$class":"JSTypeExpression","source_name":…,"root":Node}` |
| `FeatureSet` (`FEATURE_SET` on `SCRIPT`) | sorted feature-name array |
| `StaticSourceFile` (`SOURCE_FILE`) | `{"$class":"StaticSourceFile","name":…,"kind":"STRONG\|WEAK\|EXTERN"}` |
| `InputId` (`INPUT_ID`) | `{"$class":"InputId","name":…}` |
| `Map` | `{"$map":[[key,value],…]}` in iteration order |
| `Collection` | array in iteration order |
| enum | `name()` |
| any other `com.google.javascript.rhino.*` object | bean object, as for JSDocInfo |
| anything else | `{"$class":…,"$toString":…}` (not expected from the parser) |

`SOURCE_FILE` appears on every node that the parser stamped. Consumers may compress it, but
the oracle does not.

## JSDocInfo

A JSDocInfo object holds one entry for every **public, non-static, no-argument, non-void**
method of `JSDocInfo`:
- entries are sorted by method name;
- `toString`, `toStringVerbose`, `clone`, `toBuilder` and `hashCode` are excluded;
- the key is the method name and the value is the serialised return value;
- if a getter throws, its value is `{"$threw": exceptionClass}`.

These getters cover all of JSDocInfo's observable state. Examples:
- `getType`, `getReturnType`, `getParameterNames`, `getTemplateTypes`,
  `getTypeTransformations`;
- `getVisibility`, `getSuppressions`, `getSuppressionsAndTheirDescription`;
- `getMarkers` (populated only under `INCLUDE_ALL_COMMENTS`);
- `getOriginalCommentString`, `getOriginalCommentPosition`, `getLicense`, `getDescription`;
- every `is*` and `has*` flag, the `contains*Declaration` helpers, `getTypeNodes` and
  `getTypeExpressions`.

The key `"$class"` is the class name. The key `"$parameters"` lists, for each parameter in
declaration order:

```json
{"name", "type": JSTypeExpression|null, "description", "has_parameter_type"}
```

Nested rhino objects, such as `JSDocInfo.Marker` and its `StringPosition`, `TypePosition` and
`NamePosition`, are serialised with the same getter rule.

## Stability notes

- The output is deterministic for a given input and options, apart from the order of
  JSON object keys. Compare after canonical re-serialisation, for example Python's
  `json.dumps(sort_keys=True)`.
- **Known derived fields.** The `contains*` and `is*Generator` getters are computed from
  other state. They are kept because the Rust implementation must answer the same queries.
