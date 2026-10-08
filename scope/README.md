# scope/

`flags.txt` is the flag list of [docs/PORTING.md §2](../docs/PORTING.md) ("Scope"): everything
reachable from `CommandLineRunner` with the flags listed there as in scope is in scope for the
port (D-016 item 4).

## Files

| File | What it is |
|---|---|
| `flags.txt` | Generated. One row per `@Option` in `CommandLineRunner.Flags`, with the scope verdict and its reason. Do not edit it by hand. |
| `gen_flags.py` | The generator. It reads the pinned `reference/closure-compiler/src/com/google/javascript/jscomp/CommandLineRunner.java` and nothing else. |

## Commands

```bash
python3 scope/gen_flags.py           # write scope/flags.txt
python3 scope/gen_flags.py --check   # exit 1 if scope/flags.txt is not what the source gives
python3 scope/gen_flags.py --stdout  # print, do not write
```

`gates/gate_0_3.sh` runs `--check` in step 0 and stops if it fails. The Gate 0.3 report also
records the result (`scope_flags`).

## Format

The file starts with `#` header lines. These record the source path and its sha256, the counts,
the rules, and how many flags each area rule matched. After the header comes one tab-separated
row per flag, in source order:

| Column | Content |
|---|---|
| `scope` | `in` or `out` |
| `flag` | the option name, for example `--compilation_level` |
| `aliases` | comma-separated, or `-` |
| `type` | the annotated field's Java type; `boolean` when the handler is `BooleanOptionHandler` |
| `default` | the field initializer, as written in Java |
| `hidden` | `yes` or `no` (`hidden = true` in the annotation) |
| `category` | the `--help` category from `Flags.categories`, or `(uncategorized)` |
| `reason` | why the flag is in or out |
| `usage` | the usage text, with string concatenation resolved and whitespace collapsed |

`gen_flags.load()` returns the rows as dicts. The fuzz driver's `scope_table` reads the same
columns.

## Rules (docs/PORTING.md §2)

The rules are applied in order, and the first match decides:

1. The usage text says "DO NOT USE" (case-insensitive): **out**.
2. The usage text says "experimental" (case-insensitive): **out**.
3. The flag name or an alias matches an area rule (`AREA_RULES` in `gen_flags.py`): **out**.
   The areas are refactoring, lint, instrumentation, ant, debugger, J2CL, Polymer and Chrome,
   and each rule carries its evidence (the package or pass the flag configures). The header
   shows how many flags each rule matched, so a rule that matches nothing is visible. Today
   refactoring, lint, ant and debugger match no `CommandLineRunner` flag. `--debug` is not the
   `debugger/` package: it turns on debug renaming.
4. Otherwise: **in**.

The generator adds no exclusion that docs/PORTING.md §2 does not name. Adding one needs a
decision in `DECISIONS.md` (§2: the excluded code must be reachable from fewer than 0.5% of D2
runs).

At the pinned reference: 102 flags, 94 in scope and 8 out:
- `--typed_ast_output_file`: DO NOT USE.
- `--instrument_mapping_report`, `--instrument_for_coverage_option` and
  `--production_instrumentation_array_name`: instrumentation.
- `--polymer_version`: Polymer.
- `--chrome_pass`: Chrome.
- `--j2cl_pass` and `--remove_j2cl_asserts`: J2CL.

## Who uses it

- **The fuzz driver** (`fuzz/driver/src/main.rs`):
  - `OPTION_POOL`, the random option flags, draws only flags that are `in` here.
  - `NOT_DRAWN` lists every other in-scope flag with the reason the driver does not draw it.
    Examples are runner-owned argv and file layout, extra input or output files, flags that
    exit before compiling, and output that cannot be reproduced.
  - `check_option_pool` enforces both. It runs at the start of `export` and `run`, and in
    `cargo test`. A pooled flag that is out of scope or missing, or an in-scope flag that is
    neither drawn nor listed, stops the driver.
- **The Rust CLI** accepts every `in` flag.
- **The npm wrapper's TypeScript types**: `scripts/gen_npm_types.mjs` reads every row (usage,
  default, category, scope) together with the CLI's `OPTIONS` table and writes
  `npm/closure-rs/index.d.ts`. Rerun it when this file changes.

Regenerate the file whenever the pinned reference changes, because the header's sha256 will no
longer match.
