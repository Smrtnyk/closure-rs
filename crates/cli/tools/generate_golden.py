#!/usr/bin/env python3
"""Capture CLI bytes sequentially from the pinned jar (one JVM, <= 2 GiB)."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
import paths as _paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout
root=Path(__file__).resolve().parents[1]
repo=Path(_paths.ROOT)
cache=repo/'corpus-cache/cli'
metadata=json.loads((cache/'metadata.json').read_text())
fixture=root/'tests/data/cli_golden'
fixture.mkdir(parents=True,exist_ok=True)
files={'flags-valid.txt':'\ufeff--warning_level="VERBOSE" --jscomp_off="checkTypes" --help',
       'flags-quoted.txt':'--define="TEXT=hello world" --help',
       'flags-nested.txt':'--flagfile flags-valid.txt',
       'flags-invalid.txt':'--warning_level invalid',
       'at-valid.txt':'--warning_level\nverbose\n--help\n',
       'at-equals.txt':'--warning_level=verbose\r\n--help\r',
       'at-invalid.txt':'--warning_level=invalid\n',
       'wrapper.txt':'before\n%output%\nafter'}
for name,text in files.items():(fixture/name).write_text(text)
valid={'browserFeaturesetYear':'2018','compilationLevel':'WHITESPACE_ONLY','instrumentForCoverageOption':'NONE',
       'sourceMapLocationMapping':'src|dest','sourceMapInputs':'foo.js|foo.map','outputWrapper':'(%output%)',
       'outputWrapperFile':'wrapper.txt','flagFiles':'flags-valid.txt','browserResolverPrefixReplacements':'old=new','chunk':'base:0','chunkWrapper':'base:%s'}
cases=[]
def add(argv):
 if argv not in cases:cases.append(argv)
add(['--help']);add(['--help_markdown']);add(['--version']);add(['--version','a.js']);add(['a.js','--version'])
for row in metadata['flags']:
 name=row['name'];field=row['field'];typ=row['type'];handler=row['handler']
 if handler=='BooleanOptionHandler':value='false'
 elif 'enum' in row:value=row['enum'][0]
 elif typ in ['int','java.lang.Integer']:value=valid.get(field,'1')
 elif 'java.util.Map' in typ:value='old=new'
 else:value=valid.get(field,'value')
 args=[name,value,'--help']
 if field=='compilationLevel' and value=='BUNDLE':args[0:0]=['--js_output_file','bundle.js']
 add(args);add([name+'='+value,'--help'])
 if handler=='BooleanOptionHandler':add([name,'strange.js','--help'])
 elif 'enum' in row or typ in ['int','java.lang.Integer']:
  add([name,'not-a-value']);add([name+'=not-a-value'])
 elif typ != "boolean":add([name])
 for alias in row['aliases']:
  add([alias,value,'--help']);add([alias+'='+value,'--help'])
for value in ['true','on','yes','1','false','off','no','0','TRUE','YES','False','OFF','maybe','']:
 add(['--debug',value,'--help']);add(['--debug='+value,'--help'])
for value in ['simple','SIMPLE_OPTIMIZATIONS','Whitespace','whitespace_only','advanced','ADVANCED_OPTIMIZATIONS','TRANSPILE_ONLY']:
 add(['-O',value,'--help'])
for argv in [ ['--no_such_option'], ['--help','--unknown'], ['--unknown','--version'], ['--'], ['-'],
 ['--flagfile','missing-flags.txt'], ['--flagfile','flags-nested.txt'], ['--flagfile','flags-invalid.txt'],
 ['--flagfile','flags-quoted.txt'], ['@at-valid.txt'], ['@at-equals.txt'], ['@at-invalid.txt'], ['@missing-at.txt'],
 ['--output_wrapper','invalid'], ['--output_wrapper_file','missing-wrapper.txt'],
 ['--output_wrapper','%output%','--isolation_mode','IIFE'], ['--compilation_level','BUNDLE'],
 ['--compilation_level','ADVANCED','--renaming','false'], ['--dependency_mode','PRUNE'],
 ['--dependency_mode','NONE','--entry_point','goog:main'], ['--dependency_mode','SORT_ONLY','--entry_point','main'],
 ['--source_map_input','bad'], ['--source_map_location_mapping','bad'],
 ['--formatting','pretty-print','--help'], ['--warning_level','vErBoSe','--help'],
 ['--debug=true','--version'],['--version','--help'],['--help','--version'],
 ['--warning_level="VERBOSE"','--help'],['--warning_level=\'QUIET"','--help'],
 ['--js=\'foo.js\'','--help'], ['--js','"foo.js"','--help'],
 ['--jscomp_error','"checkTypes"','--help'], ['--jscomp_warning=\'deprecated\'','--help'],
 ['--jscomp_warning',"'checkTypes'",'--help'], ['--warning_level VERBOSE','--help'],
 ['--remove_j2cl_asserts=false','--help'], ['--version','--logging_level','bogus'],
 ['--browser_featureset_year','2018','--language_out','ECMASCRIPT_NEXT'],
 ]:add(argv)
# Setup errors detected before any compiler work.
for flag,values in {'language_in':['bogus','UNSUPPORTED'],'language_out':['bogus','UNSUPPORTED'],
 'j2cl_pass':['bogus'],'charset':['bogus'],'browser_featureset_year':['2011','2015','2027'],
 'chunk':['foo','foo:-1','foo:bad','foo:1:dep:extra:fifth','foo:'],
 'jscomp_error':['not_a_group'],'instrument_for_coverage_option':['PRODUCTION']}.items():
 for value in values:add(['--'+flag,value])
for value in ['bogus','warning',' Warning ','','0x10','2147483648','-2147483649']:
 add(['--logging_level',value])
for value in ['OFF','SEVERE','WARNING','INFO','CONFIG','FINE','FINER','FINEST','ALL','123','+123','-123','１２３']:
 add(['--version','--logging_level',value])
add(['--logging_level','bogus','--language_in','bogus'])
add(['--instrument_mapping_report','map.txt'])
add(['--instrument_for_coverage_option','PRODUCTION','--instrument_mapping_report','map.txt'])
add(['--chunk_output_type','ES_MODULES','--rename_prefix_namespace','root'])
add(['--chunk_output_type','ES_MODULES','--emit_use_strict'])
add(['--chunk','a:0','--chunk','b:auto'])
add(['--js','fake.js','--chunk','1invalid:1'])
add(['--js','fake.js','--chunk','a:1:missing'])
add(['--js','fake.js','--chunk','a:0'])
add(['--js','fake.js','--chunk','a:2'])
add(['--js','fake.js','--chunk','a:1','--chunk','a:0'])
# Ensure these inputs terminate before compilation. Bare '-' would reach compilation; cover it under help.
cases.remove(['-']);add(['-','--help'])
(cache/'golden-argv.json').write_text(json.dumps(cases,indent=2)+'\n')
records=[]
previous=json.loads((fixture/'cases.json').read_text()) if (fixture/'cases.json').exists() else []
previous={json.dumps(r['argv']):r for r in previous}
for i,argv in enumerate(cases):
 if json.dumps(argv) in previous:
  records.append(previous[json.dumps(argv)]);continue
 result=subprocess.run([os.environ.get('JAVA','java'),'-Xmx2g','-jar',str(repo/'build/reference/closure-compiler.jar'),*argv],input=b'',capture_output=True,cwd=fixture,env={**os.environ,'LANG':'C.UTF-8'},timeout=30)
 streams={}
 for name,value in [('stdout',result.stdout),('stderr',result.stderr)]:
  digest=hashlib.sha256(value).hexdigest()[:20];filename=digest+'.'+name
  (fixture/filename).write_bytes(value);streams[name]=filename
 records.append({'argv':argv,'exit_code':result.returncode,**streams})
 if i%50==0:print(f'{i}/{len(cases)}',flush=True)
(fixture/'cases.json').write_text(json.dumps(records,indent=2)+'\n')
(cache/'golden-records.json').write_text(json.dumps(records,indent=2)+'\n')
print(f'{len(records)} cases, {sum(p.stat().st_size for p in fixture.iterdir())} bytes')
