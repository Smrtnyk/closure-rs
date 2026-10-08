#!/usr/bin/env python3
"""Emit the minimal typed CompilerOptions data surface, preserving Java defaults."""
from pathlib import Path
import json,re,sys
root=Path(__file__).resolve().parents[1]
meta=json.loads(Path(sys.argv[1]).read_text())
src=Path(sys.argv[2]).read_text()
def snake(x): return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',x)).lower()
def lit(x): return json.dumps(x,ensure_ascii=False)
existing=(root/'src/stand_in/compiler_options.rs').read_text().split('#[derive(Clone, Debug, Default)]')[0]
known=set(re.findall(r'pub enum (\w+)',existing)) | {'Environment','Format','DetailLevel','ResolutionMode','ErrorFormat'}
extra=[]; fields=[]; unmodelled=[]
for row in meta:
 jt=row['type']; name=row['field']; d=row['default']; enum=jt.split('$')[-1].split('.')[-1]
 val=None; dump=None
 if 'enum' in row:
  if enum not in known:
   vals=row['enum']; known.add(enum)
   extra.append('#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum '+enum+' { '+', '.join(vals)+' }\nimpl '+enum+' {\n// port: '+jt.split('.')[-1]+'#valueOf\npub fn value_of(value: &str) -> Option<Self> { match value { '+', '.join(lit(v)+' => Some(Self::'+v+')' for v in vals)+', _ => None } }\n}\nimpl std::fmt::Display for '+enum+' {\n// port: Enum#toString\nfn fmt(&self,f:&mut std::fmt::Formatter<\'_>)->std::fmt::Result { write!(f,"{self:?}") }\n}')
  rt=enum; val=rt+'::'+d if d is not None else 'None'; dump='serde_json::json!(format!("{:?}", self.'+snake(name)+'))'
  if d is None: rt='Option<'+rt+'>';dump='serde_json::json!(self.'+snake(name)+'.map(|v| format!("{v:?}")))'
 elif jt=='boolean': rt='bool';val=str(d).lower()
 elif jt=='int': rt='i32';val=str(d)
 elif jt in ['java.lang.String','java.nio.file.Path']: rt='String';val=lit(d)+'.into()' if d is not None else 'None'
 elif jt=='com.google.common.base.Optional<java.lang.Boolean>': rt='Option<bool>';val='None' if d is None else 'Some('+str(d).lower()+')'
 elif jt=='com.google.common.base.Optional<com.google.javascript.jscomp.parsing.parser.FeatureSet>': rt='Option<FeatureSet>';val='None';dump='serde_json::json!(self.'+snake(name)+'.map(|v|v.to_string()))'
 elif jt=='java.nio.charset.Charset': rt='Option<Charset>';val='None';dump='serde_json::json!(self.'+snake(name)+'.map(|v|v.name()))'
 elif name=='codingConvention': rt='Option<CodingConvention>';val='None';dump='serde_json::json!(self.coding_convention.map(|v|v.java_class_name()))'
 elif name=='dependencyOptions': rt='DependencyOptions';val='DependencyOptions { mode: DependencyMode::NONE, entry_points: Vec::new() }';dump='self.dependency_options.to_json()'
 elif name=='sourceMapLocationMappings': rt='Vec<PrefixLocationMapping>';val='Vec::new()';dump='serde_json::json!(self.source_map_location_mappings.iter().map(|v|serde_json::json!({"prefix":v.prefix,"replacement":v.replacement})).collect::<Vec<_>>())'
 elif name in ['browserResolverPrefixReplacements']: rt='IndexMap<String,String>';val='IndexMap::new()'
 elif name=='defineReplacements': rt='IndexMap<String,DefineValue>';val='IndexMap::new()';dump='serde_json::Value::Object(self.define_replacements.iter().map(|(k,v)|(k.clone(),v.to_json())).collect())'
 elif re.fullmatch(r'(?:java.util.List|java.util.Set|com.google.common.collect.Immutable(?:List|Set))<java.lang.String>',jt):
  rt='Vec<String>';val='vec!['+', '.join(lit(v)+'.into()' for v in (d or []))+']'
  if d is None:rt='Option<Vec<String>>';val='None'
 else: unmodelled.append(row);continue
 if d is None and rt in ['String']: rt='Option<String>'
 if dump is None: dump='serde_json::json!(self.'+snake(name)+')'
 fields.append((name,rt,val,dump))
lines=[existing,*extra,
'use closure_parsing::parser::feature_set::FeatureSet;',
'use closure_rhino::java_lang::charset::Charset;',
'use closure_resources::compiler_options::Environment;',
'use closure_jscomp::source_map::{Format, DetailLevel, PrefixLocationMapping};',
'use closure_jscomp::error_format::ErrorFormat;',
'use super::module_loader::ResolutionMode;',
'use super::dependency_options::{DependencyOptions,DependencyMode};',
'use super::coding_convention::CodingConvention;',
'use crate::abstract_command_line_runner::FlagUsageException;',
'use indexmap::IndexMap;',
'#[derive(Clone, Debug)]','pub struct CompilerOptions {']
for name,rt,val,dump in fields: lines+=['pub '+snake(name)+': '+rt+',']
lines+=['pub warning_levels: IndexMap<String,closure_jscomp::check_level::CheckLevel>,','}', 'impl Default for CompilerOptions {','// port: CompilerOptions#CompilerOptions','fn default() -> Self { Self {']
for name,rt,val,dump in fields:lines+=[snake(name)+': '+val+',']
lines+=['warning_levels: IndexMap::new(),','} } }','impl CompilerOptions {','// port: CompilerOptions#CompilerOptions','pub fn new() -> Self { Self::default() }',
'pub fn to_json(&self) -> serde_json::Value { let mut fields=serde_json::Map::new();']
for name,rt,val,dump in fields: lines+=['fields.insert('+lit(name)+'.into(), '+dump+');']
lines+=['serde_json::Value::Object(fields) }',
'pub fn diff_against_defaults(&self) -> serde_json::Value { let current=self.to_json();let defaults=Self::new().to_json();serde_json::Value::Object(current.as_object().unwrap().iter().filter(|(k,v)|defaults.get(*k)!=Some(*v)).map(|(k,v)|(k.clone(),v.clone())).collect()) }']
custom={'setLanguageIn','setLanguageOut','setBrowserFeaturesetYear','setInlineVariables','setRemoveUnusedVariables','setIsolatePolyfills','setSmartNameRemoval','setExtractPrototypeMemberDeclarations','setEmitUseStrict','setStrictModeInput','setPolymerVersion','setExtraAnnotationNames'}
for m in re.finditer(r'public void (set\w+)\s*\(([^)]*)\)\s*\{',src):
 name,params=m.groups()
 if name in custom:continue
 end=m.end();level=1
 while level and end<len(src):
  if src[end]=='{':level+=1
  elif src[end]=='}':level-=1
  end+=1
 body=src[m.end():end-1].strip()
 if ',' in params:continue
 param=params.split()[-1]
 for fn,rt,val,dump in fields:
  if re.fullmatch(r'(?:this\.)?'+re.escape(fn)+r'\s*=\s*(?:'+re.escape(param)+r'|Immutable(?:List|Set)\.copyOf\('+re.escape(param)+r'\)|checkNotNull\('+re.escape(param)+r'\)|Optional\.of\('+re.escape(param)+r'\));',body):
   lines+=['// port: CompilerOptions#'+name,'pub fn '+snake(name)+'(&mut self, value: '+rt+') { self.'+snake(fn)+' = value; }'];break
lines+=['}']
(root/'src/stand_in/compiler_options.rs').write_text('\n'.join(lines)+'\n')
(root/'tools/options_model.json').write_text(json.dumps({'modelled':[f[0] for f in fields],'unmodelled':[{'field':r['field'],'type':r['type']} for r in unmodelled]},indent=2)+'\n')
print(len(fields),'modelled;',len(unmodelled),'unmodelled')
