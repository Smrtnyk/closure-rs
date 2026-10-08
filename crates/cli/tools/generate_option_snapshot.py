"""Generate the CLI reflection projection using public jscomp getters."""
import re,json
from pathlib import Path
s=Path('crates/jscomp/tests/support/mod.rs').read_text();s=s[s.index('pub fn options_fields'):]
keys=json.loads(Path('crates/cli/tools/options_model.json').read_text())['modelled']
rows={}
for m in re.finditer(r'\(\s*"(\w+)"\s*,',s):
 start=m.start();depth=0;quote=False;escape=False
 for end in range(start,len(s)):
  ch=s[end]
  if quote:
   if escape:escape=False
   elif ch=='\\':escape=True
   elif ch=='"':quote=False
  elif ch=='"':quote=True
  elif ch=='(':depth+=1
  elif ch==')':
   depth-=1
   if depth==0:break
 expr=s[m.end():end].strip().rstrip(',').replace('.record()', '.setup_json()')
 if m.group(1) in keys: rows[m.group(1)]=expr
header='''use closure_jscomp::compiler_options::*;
use closure_jscomp::dependency_options::DependencyOptions;
use closure_jscomp::coding_convention::CodingConvention;
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::{java_lang::charset::Charset, js_string::JsString};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Value, json};
use std::{sync::Arc, path::{Path,PathBuf}};
// port: CliOptionSetup#normalize
pub trait ValueForSetup { fn setup_json(&self) -> Value; }
impl<T: ValueForSetup + ?Sized> ValueForSetup for &T { fn setup_json(&self)->Value{(*self).setup_json()} }
impl<T: ValueForSetup + ?Sized> ValueForSetup for Arc<T> { fn setup_json(&self)->Value{self.as_ref().setup_json()} }
impl<T: ValueForSetup> ValueForSetup for Option<T> { fn setup_json(&self)->Value{self.as_ref().map_or(Value::Null, ValueForSetup::setup_json)} }
impl<T: ValueForSetup> ValueForSetup for Vec<T> { fn setup_json(&self)->Value{Value::Array(self.iter().map(ValueForSetup::setup_json).collect())} }
impl<T: ValueForSetup> ValueForSetup for IndexSet<T> { fn setup_json(&self)->Value{Value::Array(self.iter().map(ValueForSetup::setup_json).collect())} }
impl<T: ValueForSetup> ValueForSetup for IndexMap<String,T> { fn setup_json(&self)->Value{Value::Object(self.iter().map(|(k,v)|(k.clone(),v.setup_json())).collect())} }
macro_rules! primitive { ($($type:ty),*)=>{$(impl ValueForSetup for $type{ fn setup_json(&self)->Value{json!(self)}})*}; }
primitive!(bool,i32,str,String,f64);
impl ValueForSetup for JsString {fn setup_json(&self)->Value{json!(self.to_string_lossy())}}
impl ValueForSetup for Path {fn setup_json(&self)->Value{json!(self.to_string_lossy())}}
impl ValueForSetup for PathBuf {fn setup_json(&self)->Value{self.as_path().setup_json()}}
impl ValueForSetup for FeatureSet {fn setup_json(&self)->Value{json!(self.to_string())}}
impl ValueForSetup for BrowserFeaturesetYear {fn setup_json(&self)->Value{json!(format!("YEAR_{}",self.get_year()))}}
impl ValueForSetup for Charset {fn setup_json(&self)->Value{json!(self.name())}}
impl ValueForSetup for DependencyOptions {fn setup_json(&self)->Value{json!({"mode":self.mode().to_string(),"entryPoints":self.entry_points().iter().map(ToString::to_string).collect::<Vec<_>>()})}}
impl ValueForSetup for DefineValue {fn setup_json(&self)->Value{match self{Self::Boolean(v)=>v.setup_json(),Self::Integer(v)=>v.setup_json(),Self::Double(v)=>v.setup_json(),Self::String(v)=>v.setup_json()}}}
// port: CliOptionSetup#normalize
pub fn coding_convention_class_name(value:&dyn CodingConvention)-> &'static str {
 let debug=format!("{value:?}");
 if debug.starts_with("ClosureCodingConvention"){"ClosureCodingConvention"}
 else if debug.starts_with("ChromeCodingConvention"){"ChromeCodingConvention"}
 else {"DefaultCodingConvention"}
}
impl ValueForSetup for dyn CodingConvention + Send + Sync {fn setup_json(&self)->Value{json!(coding_convention_class_name(self))}}
macro_rules! enums { ($($type:ty),*)=>{$(impl ValueForSetup for $type{fn setup_json(&self)->Value{json!(format!("{self:?}"))}})*}; }
enums!(LanguageMode, Environment, IncrementalCheckMode, DevMode, ExtractPrototypeMemberDeclarationsMode, AliasStringsMode, OptimizeLocalAccess, PropertyCollapseLevel, J2clPassMode, TweakProcessing, OutputJs, Es6SubclassTranspilation, TracerMode, InstrumentOption, ConformanceReportingMode, ChunkOutputType, Reach, Es6ModuleTranspilation, closure_parsing::config::JsDocParsing, closure_rhino::jscomp_base::Tri, closure_jscomp::variable_renaming_policy::VariableRenamingPolicy,closure_jscomp::property_renaming_policy::PropertyRenamingPolicy,closure_jscomp::js::runtime_js_lib_manager::RuntimeLibraryMode, closure_jscomp::error_format::ErrorFormat,closure_jscomp::source_map::DetailLevel,closure_jscomp::source_map::Format,closure_jscomp::deps::module_loader::ResolutionMode,closure_jscomp::deps::module_loader::PathEscaper);
fn guava_optional<T:ValueForSetup>(value:&Option<T>)->Value{value.setup_json()}
fn implementation(value:Value,_class:&str)->Value{value}
// port: CliOptionSetup#fields
pub fn options_fields(options:&CompilerOptions)->Value {
 Value::Object([
'''
p=Path('crates/cli/src/option_setup.rs')
p.write_text(header+'\n'.join(f'("{k}".into(),{rows[k]}),' for k in keys)+'''\n].into_iter().collect())
}
// port: CliOptionSetup#capture
pub fn diff_against_defaults(options:&CompilerOptions)->Value{
 let defaults=options_fields(&CompilerOptions::new());
 let mut values=options_fields(options);
 values.as_object_mut().unwrap().retain(|k,v|defaults.get(k)!=Some(v));
 values
}
''')
