# Printer differential fixtures

These gzip JSON fixtures contain Java AST dumps and reference CodePrinter results. The Rust
loader rebuilds the same AST, metadata, comments and JSDocInfo; the test compares UTF-16 source
units, source-map JSON and collected licenses exactly. Every dumped NumberNode is also checked
against Java's `double_closure` result. Java rejection is checked by exception class and message.

There are 120 fixtures (1,134,443 compressed bytes): 95 corpus inputs and 25 supplemental probes.
They run 4,680 configuration comparisons, including 833 source-map JSON comparisons and 375
individual number checks. The UNDEFINED_TYPE probe accounts for 39 expected rejection results:
the reference CodeGenerator has no printing arm for this token.

The complete sweep has 3,520 distinct successfully parsed D2 inputs. Java selected 3,600 paths
and rejected 80 with parser errors. It uses OracleRunner.createOptionsForParseDump with UNSTABLE
input and INCLUDE_ALL_COMMENTS, the reference jar, oracle_client.golden_env(), JDK 21, -Xmx2g,
and -Xss64m. The oracle jar and oracle dump implementation are unchanged; PrinterGolden adds
metadata to its own dump.

Every fixture and sweep input runs the same 39 configurations:

- compact, line_break, cut_40, map_all, map_symbols, single_quotes, ascii, utf8, latin1;
- trusted, untrusted, strict, original_names, es5, es2015, es2019;
- pretty, pretty_map_all, pretty_cut_40, pretty_line_break, quote_keywords, type_summary;
- pretty_single_quotes_latin1, comments, pretty_comments, gents, es3, uncut_map, map_path;
- builder_pretty, builder_line_break, utf16, utf16be, utf16le, ijs_keywords;
- types, pretty_types, output_types, licenses.

The probes cover operators, control flow, functions, classes, modules, patterns, optional chains,
line cuts, numbers, all ASCII code units, representative Unicode units and lone surrogates,
original names (including lone-surrogate source-map symbols), leading/inline/trailing comments,
source-backed number bounds and mismatch/fallback cases, stub source files, direct consumer
line-cut/map-disabled behavior, UTF-16 source-map filenames/contents/wrapper prefixes, and custom
generator virtual dispatch.
Hand-built ASTs add declared types, generic parameters, optional parameters, interface members,
index/call/construct signatures, class implements/extends, enum, namespace, declare, type aliases,
CAST and closure-unaware shadows. ACCESS_MODIFIER fixtures remain blocked because rhino Visibility
has no Display implementation required by OpaqueProp. Node subclass preconditions remain blocked
because rhino has no plain-node/NodeKind accessor.

Non-JSDoc comments retain UTF-16 text, source positions and inline/line-comment flags. The JSDoc
loader reconstructs all public bean fields through JSDocInfo.Builder and checks every dumped
public getter against the rebuilt object, including ordered maps, descriptions, marker positions,
parameterized getters and type ASTs. Type-expression implicit source names are preserved.
`types` and `pretty_types` therefore exercise JSDocInfoPrinter inside CodeGenerator. `output_types`
selects TypedCodeGenerator with a real JSTypeRegistry; these dumps do not contain inferred JSTypes.
The ten hand-built CodePrinterEs6TypedTest cases separately verify inline type declarations.

The Java helper and manifests that produced the fixtures are not tracked (they live in the
gitignored `build/` directory). The larger golden cache is under `corpus-cache/printer/` of the
main checkout. Run the full sweep with:

```sh
export CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true
source scripts/paths.sh   # ROOT: the main checkout
CLOSURE_RS_PRINTER_DIFF_DIR="$ROOT/corpus-cache/printer/sweep" \
  cargo test -p closure-jscomp --test code_printer_diff_test -- --nocapture
```

Without this variable, cargo test runs all committed fixtures. No configurations or expected
rejections are filtered out.
