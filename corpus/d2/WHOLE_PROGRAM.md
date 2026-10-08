# Whole-program cases: library tests on Java's output

Each whole-program case's `test_command` (`node corpus/d2/shims/<id>/run.mjs {output}`) was run against the golden Java output of every profile of the case (`corpus/d2/validate/wp_test.py`, node v24.18.0). For a chunk profile (`chunks2`, `chunks3`), `{output}` is the chunk outputs concatenated in chunk order with a newline between them (`c0.js`, `c1.js`[, `c2.js`]), i.e. the chunks loaded as consecutive scripts.

A case's `test_profiles` lists the profiles whose Java output passes the library's own tests. Only those profiles carry the test requirement (D2: Rust output must pass too). Profiles whose Java output fails keep the byte-for-byte output comparison but have no test requirement.

| Case | Result per profile (passed/tests) | test_profiles |
|---|---|---|
| `whole-program-acorn-8.19.0` | simple: pass (2/2), advanced: FAIL (0/2), lang_es5: pass (2/2), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (2/2), lang_next: pass (2/2), pretty: pass (2/2), sourcemap: pass (2/2), chunks2: FAIL (0/2), chunks3: FAIL (0/2) | simple, lang_es5, lang_es2015, lang_next, pretty, sourcemap |
| `whole-program-base64-js-1.5.1` | simple: pass (6/6), advanced: pass (6/6), lang_es5: pass (6/6), ws: pass (6/6), advanced_strict: FAIL, lang_es2015: pass (6/6), lang_next: pass (6/6), pretty: pass (6/6), sourcemap: pass (6/6), chunks2: pass (6/6), chunks3: pass (6/6) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-big.js-7.0.1` | simple: pass (17/17), advanced: pass (17/17), lang_es5: pass (17/17), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (17/17), lang_next: pass (17/17), pretty: pass (17/17), sourcemap: pass (17/17), chunks2: pass (17/17), chunks3: pass (17/17) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-bignumber.js-11.1.5` | simple: pass (34/34), advanced: pass (34/34), lang_es5: pass (34/34), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (34/34), lang_next: pass (34/34), pretty: pass (34/34), sourcemap: pass (34/34), chunks2: pass (34/34), chunks3: pass (34/34) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-bytes-3.1.2` | simple: pass (30/30), advanced: pass (30/30), lang_es5: pass (30/30), ws: pass (30/30), advanced_strict: FAIL, lang_es2015: pass (30/30), lang_next: pass (30/30), pretty: pass (30/30), sourcemap: pass (30/30), chunks2: pass (30/30), chunks3: pass (30/30) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-camelcase-9.0.0` | simple: pass (20/20), advanced: pass (20/20), lang_es5: pass (20/20), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (20/20), lang_next: pass (20/20), pretty: pass (20/20), sourcemap: pass (20/20), chunks2: pass (20/20), chunks3: pass (20/20) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-classnames-2.5.1` | simple: pass (63/63), advanced: pass (63/63), lang_es5: pass (63/63), ws: pass (63/63), advanced_strict: FAIL, lang_es2015: pass (63/63), lang_next: pass (63/63), pretty: pass (63/63), sourcemap: pass (63/63), chunks2: pass (63/63), chunks3: pass (63/63) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-decimal.js-10.6.0` | simple: pass (60/60), advanced: pass (60/60), lang_es5: pass (60/60), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (60/60), lang_next: pass (60/60), pretty: pass (60/60), sourcemap: pass (60/60), chunks2: pass (60/60), chunks3: pass (60/60) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-dequal-2.0.3` | simple: pass (44/44), advanced: pass (44/44), lang_es5: pass (44/44), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (44/44), lang_next: pass (44/44), pretty: pass (44/44), sourcemap: pass (44/44), chunks2: pass (44/44), chunks3: pass (44/44) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-escape-string-regexp-5.0.0` | simple: pass (3/3), advanced: pass (3/3), lang_es5: pass (3/3), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (3/3), lang_next: pass (3/3), pretty: pass (3/3), sourcemap: pass (3/3), chunks2: pass (3/3), chunks3: pass (3/3) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-fast-json-stable-stringify-2.1.0` | simple: pass (16/16), advanced: pass (16/16), lang_es5: pass (16/16), ws: pass (16/16), advanced_strict: FAIL, lang_es2015: pass (16/16), lang_next: pass (16/16), pretty: pass (16/16), sourcemap: pass (16/16), chunks2: pass (16/16), chunks3: pass (16/16) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-ieee754-1.2.1` | simple: pass (4/4), advanced: pass (4/4), lang_es5: pass (4/4), ws: pass (4/4), advanced_strict: FAIL, lang_es2015: pass (4/4), lang_next: pass (4/4), pretty: pass (4/4), sourcemap: pass (4/4), chunks2: pass (4/4), chunks3: pass (4/4) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-jsesc-3.1.0` | simple: pass (2/2), advanced: pass (2/2), lang_es5: pass (2/2), ws: pass (2/2), advanced_strict: FAIL, lang_es2015: pass (2/2), lang_next: pass (2/2), pretty: pass (2/2), sourcemap: pass (2/2), chunks2: pass (2/2), chunks3: pass (2/2) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-leven-4.1.0` | simple: pass (3/3), advanced: pass (3/3), lang_es5: pass (3/3), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (3/3), lang_next: pass (3/3), pretty: pass (3/3), sourcemap: pass (3/3), chunks2: pass (3/3), chunks3: pass (3/3) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-minimist-1.2.8` | simple: pass (61/61), advanced: pass (61/61), lang_es5: pass (61/61), ws: pass (61/61), advanced_strict: FAIL, lang_es2015: pass (61/61), lang_next: pass (61/61), pretty: pass (61/61), sourcemap: pass (61/61), chunks2: pass (61/61), chunks3: pass (61/61) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-mri-1.2.0` | simple: pass (23/23), advanced: pass (23/23), lang_es5: pass (23/23), ws: FAIL, advanced_strict: FAIL, lang_es2015: pass (23/23), lang_next: pass (23/23), pretty: pass (23/23), sourcemap: pass (23/23), chunks2: pass (23/23), chunks3: pass (23/23) | simple, advanced, lang_es5, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-ms-2.1.3` | simple: pass (49/49), advanced: pass (49/49), lang_es5: pass (49/49), ws: pass (49/49), advanced_strict: FAIL, lang_es2015: pass (49/49), lang_next: pass (49/49), pretty: pass (49/49), sourcemap: pass (49/49), chunks2: pass (49/49), chunks3: pass (49/49) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |
| `whole-program-punycode-2.3.1` | simple: pass (139/139), advanced: pass (139/139), lang_es5: pass (139/139), ws: pass (139/139), advanced_strict: FAIL, lang_es2015: pass (139/139), lang_next: pass (139/139), pretty: pass (139/139), sourcemap: pass (139/139), chunks2: pass (139/139), chunks3: pass (139/139) | simple, advanced, lang_es5, ws, lang_es2015, lang_next, pretty, sourcemap, chunks2, chunks3 |

## Failures (30 pairs)

### `whole-program-acorn-8.19.0` / `advanced`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): [2, 0, 2, 0].
- `not ok 1 - Normal`
- `not ok 2 - Normal with sourceType: commonjs`

### `whole-program-acorn-8.19.0` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-acorn-8.19.0` / `advanced_strict`

- Java exit code 127; test exit None; tests (total, pass, fail, skip): None.
- Java exit 127, outputs=[]

### `whole-program-acorn-8.19.0` / `chunks2`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): [2, 0, 2, 0].
- `not ok 1 - Normal`
- `not ok 2 - Normal with sourceType: commonjs`

### `whole-program-acorn-8.19.0` / `chunks3`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): [2, 0, 2, 0].
- `not ok 1 - Normal`
- `not ok 2 - Normal with sourceType: commonjs`

### `whole-program-base64-js-1.5.1` / `advanced_strict`

- Java exit code 121; test exit None; tests (total, pass, fail, skip): None.
- Java exit 121, outputs=[]

### `whole-program-big.js-7.0.1` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-big.js-7.0.1` / `advanced_strict`

- Java exit code 98; test exit None; tests (total, pass, fail, skip): None.
- Java exit 98, outputs=[]

### `whole-program-bignumber.js-11.1.5` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-bignumber.js-11.1.5` / `advanced_strict`

- Java exit code 127; test exit None; tests (total, pass, fail, skip): None.
- Java exit 127, outputs=[]

### `whole-program-bytes-3.1.2` / `advanced_strict`

- Java exit code 42; test exit None; tests (total, pass, fail, skip): None.
- Java exit 42, outputs=[]

### `whole-program-camelcase-9.0.0` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-camelcase-9.0.0` / `advanced_strict`

- Java exit code 12; test exit None; tests (total, pass, fail, skip): None.
- Java exit 12, outputs=[]

### `whole-program-classnames-2.5.1` / `advanced_strict`

- Java exit code 52; test exit None; tests (total, pass, fail, skip): None.
- Java exit 52, outputs=[]

### `whole-program-decimal.js-10.6.0` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-decimal.js-10.6.0` / `advanced_strict`

- Java exit code 127; test exit None; tests (total, pass, fail, skip): None.
- Java exit 127, outputs=[]

### `whole-program-dequal-2.0.3` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-dequal-2.0.3` / `advanced_strict`

- Java exit code 71; test exit None; tests (total, pass, fail, skip): None.
- Java exit 71, outputs=[]

### `whole-program-escape-string-regexp-5.0.0` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-escape-string-regexp-5.0.0` / `advanced_strict`

- Java exit code 2; test exit None; tests (total, pass, fail, skip): None.
- Java exit 2, outputs=[]

### `whole-program-fast-json-stable-stringify-2.1.0` / `advanced_strict`

- Java exit code 15; test exit None; tests (total, pass, fail, skip): None.
- Java exit 15, outputs=[]

### `whole-program-ieee754-1.2.1` / `advanced_strict`

- Java exit code 77; test exit None; tests (total, pass, fail, skip): None.
- Java exit 77, outputs=[]

### `whole-program-jsesc-3.1.0` / `advanced_strict`

- Java exit code 39; test exit None; tests (total, pass, fail, skip): None.
- Java exit 39, outputs=[]

### `whole-program-leven-4.1.0` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-leven-4.1.0` / `advanced_strict`

- Java exit code 5; test exit None; tests (total, pass, fail, skip): None.
- Java exit 5, outputs=[]

### `whole-program-minimist-1.2.8` / `advanced_strict`

- Java exit code 36; test exit None; tests (total, pass, fail, skip): None.
- Java exit 36, outputs=[]

### `whole-program-mri-1.2.0` / `ws`

- Java exit code 0; test exit 1; tests (total, pass, fail, skip): None.
- `not ok 1 - load the program`
- First error: `SyntaxError: Unexpected token 'export'`

### `whole-program-mri-1.2.0` / `advanced_strict`

- Java exit code 10; test exit None; tests (total, pass, fail, skip): None.
- Java exit 10, outputs=[]

### `whole-program-ms-2.1.3` / `advanced_strict`

- Java exit code 67; test exit None; tests (total, pass, fail, skip): None.
- Java exit 67, outputs=[]

### `whole-program-punycode-2.3.1` / `advanced_strict`

- Java exit code 95; test exit None; tests (total, pass, fail, skip): None.
- Java exit 95, outputs=[]

acorn: failing profiles `advanced`, `advanced_strict`, `chunks2`, `chunks3`, `ws`. Under ADVANCED (`advanced`, `chunks2`) the compiled parser throws `TypeError: this is not a constructor` on the first `parse()` call, so both test groups fail. This is the reference's ADVANCED output, not a harness problem.

## Count

18 of 18 whole programs keep a test requirement on at least one profile (the corpus requires 10 or more).
