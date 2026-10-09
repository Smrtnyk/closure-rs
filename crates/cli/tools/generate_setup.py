#!/usr/bin/env python3
"""Capture config and all options diffs, then explicitly mark unmodelled fields."""
import json,os,subprocess
from pathlib import Path
import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
import paths as _paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout
root=Path(__file__).resolve().parents[1];repo=Path(_paths.ROOT);cache=repo/'corpus-cache/cli';fixture=root/'tests/data/cli_golden'
argv=json.loads((cache/'golden-argv.json').read_text())
valid=[[],['--compilation_level','WHITESPACE_ONLY'],['-O','ADVANCED'],['-O','TRANSPILE_ONLY'],['-O','BUNDLE','--js_output_file','bundle.js'],
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
for year in [2012,*range(2018,2027)]: valid.append(['--browser_featureset_year',str(year)])
for language in ['ECMASCRIPT3','ECMASCRIPT5','ECMASCRIPT5_STRICT','ES6','ES_2017','STABLE','NO_TRANSPILE','UNSTABLE']:
 valid.append(['--language_out',language]);
 if language!='NO_TRANSPILE':valid.append(['--language_in',language])
argv+=valid
cmd=[os.environ.get('JAVA','java'),'-Xmx2g','-cp',str(repo/'build/codex/cli')+':'+_paths.REF_JAR,'com.google.javascript.jscomp.CliOptionSetup']
r=subprocess.run(cmd,input=json.dumps(argv).encode(),capture_output=True,cwd=fixture,check=True)
(cache/'option-setup-raw.json').write_bytes(r.stdout)
rows=json.loads(r.stdout)
for row in rows:
 row['setup'].pop('stdout');row['setup'].pop('stderr')
(root/'tests/data/option_setup.json').write_text(json.dumps(rows,indent=2)+'\n')
print(len(rows),'setup cases')
