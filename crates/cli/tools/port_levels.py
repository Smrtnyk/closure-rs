#!/usr/bin/env python3
"""Mechanical translation of the pinned CompilationLevel option-setting bodies."""
from pathlib import Path
import re,sys
root=Path(__file__).resolve().parents[1]
src=Path(sys.argv[1]).read_text()
def snake(x):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',x)).lower()
out=[]
for method in ['applyBasicCompilationOptions','applyTranspileOnlyOptions','applySafeCompilationOptions','applyFullCompilationOptions']:
 m=re.search(r'private static void '+method+r'\(CompilerOptions options\) \{',src)
 i=m.end();depth=1;j=i
 while depth:
  if src[j]=='{':depth+=1
  if src[j]=='}':depth-=1
  j+=1
 body=src[i:j-1];body=re.sub(r'/\*.*?\*/|//[^\n]*','',body,flags=re.S)
 lines=[]
 for call in re.finditer(r'options\.(\w+)\((.*?)\);',body,re.S):
  name,args=call.groups();args=' '.join(args.split())
  args=args.replace('DependencyOptions.sortOnly()','DependencyOptions::sort_only()')
  args=re.sub(r'DiagnosticGroups\.(\w+)',lambda m:'"'+m[1]+'"',args)
  args=re.sub(r'(\w+)\.(\w+)',r'\1::\2',args)
  if name=='setExtractPrototypeMemberDeclarations':name='setExtractPrototypeMemberDeclarationsEnabled'
  if name=='setInlineVariables':name='setInlineVariablesReach'
  if name=='setDeadPropertyAssignmentElimination':args='Tri::FALSE'
  lines+=['options.'+snake(name)+'('+args+');']
 out+=['// port: CompilationLevel#'+method,'fn '+snake(method)+'(options: &mut CompilerOptions) {',*lines,'}']
p=root/'src/stand_in/compilation_level.rs';p.write_text(p.read_text()+'\nuse super::compiler_options::*;\nuse super::dependency_options::DependencyOptions;\nuse closure_jscomp::check_level::CheckLevel;\nimpl CompilationLevel {\n'+'\n'.join(out)+'\n}\n')
