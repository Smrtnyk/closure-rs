#!/usr/bin/env python3
"""Writes the option setup fixture (crates/cli/tests/data/option_setup.json) from the reference jar.

  python3 crates/cli/tools/generate_setup.py [--work DIR] [--check]

For every argument vector it records what CommandLineRunner sets up without compiling
(CliOptionSetup.java): whether the compiler would run, the CommandLineConfig and the CompilerOptions
fields that differ from their defaults, or the exception. The vectors are the CLI golden cases
(generate_golden.py's golden_cases, from the reference's flag list), the setups of HAND and one
setup per boolean flag that neither uses: `--flag`, or `--flag false` when the flag is on by
default. It runs CliOptionSetup compiled against the reference jar (scripts/paths.py:
$CLOSURE_RS_REF, else the registry's default) with the pinned JDK, in the CLI golden directory
(the paths it captures are relative to it), with a fixed environment, and lists the vectors added,
removed and changed. --check writes nothing and exits 1 when the fixture differs. The step
cli-fixtures of scripts/upstream_sync.py runs it.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import generate_golden  # noqa: E402
import reference_tools as rt  # noqa: E402

FIXTURE = rt.FIXTURES / 'option_setup.json'
# Setups that reach the compiler options. A boolean flag none of them uses gets its own setup
# automatically (boolean_setups), so a new boolean flag needs no entry here.
HAND = [[],['--compilation_level','WHITESPACE_ONLY'],['-O','ADVANCED'],['-O','TRANSPILE_ONLY'],['-O','BUNDLE','--js_output_file','bundle.js'],
 ['-W','QUIET'],['-W','VERBOSE'],['--debug'],['--checks-only'],['--use_types_for_optimization','false'],
 ['--assume_function_wrapper'],['--isolation_mode','IIFE'],['--chunk_output_type','ES_MODULES'],
 ['--chunk_output_type','ES_MODULES','-O','ADVANCED'],['--generate_exports','false'],['--export_local_property_definitions','false'],
 ['--formatting','PRETTY_PRINT','--formatting','PRINT_INPUT_DELIMITER','--formatting','SINGLE_QUOTES'],
 ['--process_common_js_modules'],['--process_closure_primitives','false'],['--angular_pass'],['--polymer_version','1'],['--polymer_version','2'],['--chrome_pass'],
 ['--j2cl_pass','OFF'],['--rename_variable_prefix','prefix'],['--rename_prefix_namespace','name'],['--preserve_type_annotations'],
 ['--inject_libraries','false'],['--force_inject_library','base'],['--rewrite_polyfills','false'],['--isolate_polyfills'],
 ['--strict_mode_input','false'],['--emit_use_strict'],['--source_map_include_content'],['--module_resolution','NODE'],
 ['--browser_resolver_prefix_replacements','a=b','--browser_resolver_prefix_replacements','b=c'],['--package_json_entry_names','module,main'],
 ['--renaming','false'],['--instrument_for_coverage_option','LINE'],['--instrument_for_coverage_option','BRANCH'],
 ['--instrument_for_coverage_option','PRODUCTION','--instrument_mapping_report','map.txt','--production_instrumentation_array_name','coverage'],
 ['--allow_dynamic_import','false'],['--dynamic_import_alias','load'],['--assume_static_inheritance_is_not_used','false'],['--assume_no_prototype_method_enumeration'],
 ['--js','foo.js','--js','bar.js','--chunk','foo:1','--chunk','bar:1:foo','--chunk_wrapper','foo:(%s)'],
 ['--jscomp_error','checkTypes','--jscomp_off','checkTypes','--jscomp_warning','deprecated'],
 ['--jscomp_error','*'],['--hide_warnings_for','vendor'],['--third_party'],['--extra_annotation_name','custom'],
 ['--json_streams','OUT'],['--json_streams','IN'],['--charset','UTF-8'],['--charset','ISO-8859-1'],['--output_manifest','manifest.txt'],
 ['--create_source_map','%outname%.map'],['--source_map_location_mapping','src|dest'],['--source_map_input','foo.js|foo.map'],
 ['--apply_input_source_maps','false','--parse_inline_source_maps','false'],['--dependency_mode','SORT_ONLY'],['--entry_point','goog:main'],
 ['--dependency_mode','PRUNE','--entry_point','src/main.js'],['--entry_point','goog:module:name'],
 ['--summary_detail_level','3'],['--num_parallel_threads','2'],['--dev_mode','EVERY_PASS'],['--print_tree'],
 ['--incremental_check_mode','GENERATE_IJS'],['--typed_ast_output_file','ast.bin'],['--continue_after_errors'],
 ['--define','BOOL','--define','FALSE=false','--define','NUMBER=1.5','--define',"STR='hello'",'--define','BARE=word'],
 ['--filename_to_save_to','save.bin','--segment_of_compilation_to_run','CHECKS'],
 ['--filename_to_restore_from','save.bin','--segment_of_compilation_to_run','OPTIMIZATIONS'],
 ['--filename_to_restore_from','save.bin','--segment_of_compilation_to_run','FINALIZATIONS'],
]


def hand_setups():
    """HAND, then every browser featureset year and language mode."""
    valid = [list(v) for v in HAND]
    for year in [2012, *range(2018, 2027)]:
        valid.append(['--browser_featureset_year', str(year)])
    for language in ['ECMASCRIPT3', 'ECMASCRIPT5', 'ECMASCRIPT5_STRICT', 'ES6', 'ES_2017', 'STABLE', 'NO_TRANSPILE', 'UNSTABLE']:
        valid.append(['--language_out', language])
        if language != 'NO_TRANSPILE':
            valid.append(['--language_in', language])
    return valid


def boolean_setups(metadata, used):
    """`--flag` (`--flag false` when on by default) for each boolean flag no vector in `used` names."""
    out = []
    for row in metadata['flags']:
        if row['handler'] != 'BooleanOptionHandler':
            continue
        names = [row['name'], *row['aliases']]
        if any(arg in names or arg.split('=', 1)[0] in names for argv in used for arg in argv):
            continue
        out.append([row['name']] if not row['default'] else [row['name'], 'false'])
    return out


def setups(metadata):
    golden = generate_golden.golden_cases(metadata)
    hand = hand_setups()
    # The golden cases name every flag (with --help), so only HAND decides what a flag still needs.
    automatic = [v for v in boolean_setups(metadata, hand) if v not in golden]
    return golden + hand + automatic


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--work', type=Path, help='work directory (default build/cli-fixtures/<tag>/ of this checkout)')
    ap.add_argument('--check', action='store_true', help='write nothing; exit 1 when the fixture differs')
    a = ap.parse_args()
    work = a.work or rt.default_work()
    work.mkdir(parents=True, exist_ok=True)
    argv = setups(rt.flag_metadata(work))
    cmd = [str(rt.JAVA), '-Xmx2g', '-cp', f'{rt.classes(work)}:{rt.REF_JAR}', 'com.google.javascript.jscomp.CliOptionSetup']
    r = subprocess.run(cmd, input=json.dumps(argv).encode(), capture_output=True, cwd=rt.FIXTURES / 'cli_golden',
                       env=rt.child_env(), check=True)
    rows = json.loads(r.stdout)
    for row in rows:
        row['setup'].pop('stdout')
        row['setup'].pop('stderr')
    old = json.loads(FIXTURE.read_text()) if FIXTURE.exists() else []
    old_by_argv = {json.dumps(o['argv']): o for o in old}
    new_argv = {json.dumps(row['argv']) for row in rows}
    added = [row['argv'] for row in rows if json.dumps(row['argv']) not in old_by_argv]
    removed = [o['argv'] for o in old if json.dumps(o['argv']) not in new_argv]
    differ = [row['argv'] for row in rows if json.dumps(row['argv']) in old_by_argv
              and old_by_argv[json.dumps(row['argv'])] != row]
    changed = rt.write_if_changed(FIXTURE, (json.dumps(rows, indent=2) + '\n').encode(), a.check)
    print(f'{FIXTURE.relative_to(rt.CHECKOUT)}: {len(rows)} setups of {rt.REF_TAG}; {len(added)} added, '
          f'{len(removed)} removed, {len(differ)} with another setup')
    for what, vectors in (('added', added), ('removed', removed), ('setup changed', differ)):
        for v in vectors[:40]:
            print(f'  {what}: {json.dumps(v)}')
        if len(vectors) > 40:
            print(f'  ... and {len(vectors) - 40} more {what}')
    if a.check and changed:
        print(f'the fixture differs from {rt.REF_TAG} (python3 crates/cli/tools/generate_setup.py rewrites it)')
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
