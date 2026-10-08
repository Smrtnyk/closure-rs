#!/usr/bin/env python3
"""Translate CommandLineConfig's data declarations and plain setters in Java order."""
from pathlib import Path
import re
import sys
root=Path(__file__).resolve().parents[1]
text=Path(sys.argv[1]).read_text().split('protected static class CommandLineConfig {',1)[1].split('/** Representation of a source file',1)[0]
def snake(x):
    x=x.replace('Allowlist','AllowList')
    return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',x)).lower()
fields=[]
for m in re.finditer(r'private\s+(?:final\s+)?(?:@Nullable\s+)?([\w.<>, ]+?)\s+(\w+)\s*(?:=\s*([^;]+))?;',text):
    jt,name,default=m.groups(); jt=jt.strip(); default=(default or ('false' if jt=='boolean' else 'null')).strip()
    if jt=='boolean': rt='bool'; val=default
    elif jt in ['int','Integer']: rt='i32'; val=default
    elif jt=='String': rt='String'; val=default+'.into()' if default!='null' else 'None'
    elif jt in ['List<String>']: rt='Vec<String>'; val='vec!["./".into()]' if 'DEFAULT_FILENAME_PREFIX' in default else 'Vec::new()'
    elif jt=='Map<String, String>': rt='IndexMap<String, String>'; val='IndexMap::new()'
    elif 'FlagEntry<CheckLevel>' in jt: rt='Vec<FlagEntry<CheckLevel>>'; val='Vec::new()'
    elif 'FlagEntry<JsSourceType>' in jt: rt='Vec<FlagEntry<JsSourceType>>'; val='Vec::new()'
    elif 'SourceMap.LocationMapping' in jt: rt='Vec<PrefixLocationMapping>'; val='Vec::new()'
    elif jt=='CodingConvention': rt='CodingConvention'; val='CodingConvention::Default'
    elif jt=='DependencyOptions': rt='DependencyOptions'; val='None'
    else: rt=jt.replace('CompilerOptions.','').replace('SourceMap.',''); val=default.replace('CompilerOptions.','').replace('SourceMap.','').replace('.', '::')
    if default=='Level.WARNING.getName()': val='"WARNING".into()'
    if default=='null': rt='Option<'+rt+'>'; val='None'
    fields.append((name,rt,val))
lines=['// Generated from AbstractCommandLineRunner.CommandLineConfig declarations.',
       '#[derive(Clone, Debug)]','pub struct CommandLineConfig {']
for name,rt,val in fields: lines+=['pub '+snake(name)+': '+rt+',']
lines+=['}', 'impl Default for CommandLineConfig {', '// port: AbstractCommandLineRunner.CommandLineConfig#CommandLineConfig','fn default() -> Self { Self {']
for name,rt,val in fields: lines += [snake(name)+': '+val+',']
lines+=['} }','}', 'impl CommandLineConfig {']
# Plain setters and mutable-list clear/addAll copies, retaining each method's name.
for m in re.finditer(r'(?:public\s+)?CommandLineConfig\s+(set\w+)\s*\(([^)]*)\)\s*\{([^}]+)\}',text):
    method,params,body=m.groups()
    if method in ['setDefaultToStdin','setContinueSavedCompilationFileName','setSaveCompilationStateToFilename','setOutputManifest']: continue
    params=re.sub(r'@Nullable\s+','',params).strip()
    p=re.match(r'(.+)\s+(\w+)$',params)
    if not p: continue
    ass=re.search(r'(?:this\.)?(\w+)\s*=\s*'+re.escape(p[2])+r'\s*;',body)
    clear=re.search(r'this\.(\w+)\.clear\(\)',body)
    field=ass[1] if ass else clear[1] if clear else None
    if not field: continue
    rt=next(rt for name,rt,val in fields if name==field)
    lines+=['// port: AbstractCommandLineRunner.CommandLineConfig#'+method,
            'pub fn '+snake(method)+'(&mut self, value: '+rt+') -> &mut Self { self.'+snake(field)+' = value; self }']
lines+=['}']
(root/'src/command_line_config.rs').write_text('\n'.join(lines)+'\n')
print(len(fields),'config fields')
