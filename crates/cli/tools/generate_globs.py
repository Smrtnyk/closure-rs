"""Record the pinned JDK's Unix glob matcher without using compiler passes."""
import json,os,subprocess
from pathlib import Path
import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
import paths as _paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout
repo=Path(_paths.ROOT);root=Path(__file__).resolve().parents[1];cache=repo/'corpus-cache/cli';classes=cache/'classes';jar=repo/'build/reference/closure-compiler.jar'
subprocess.run(['javac','-cp',str(jar),'-d',str(classes),str(root/'tools/CliGlob.java')],check=True)
patterns=['','*','**','***','?','??','*.js','**.js','**/*.js','foo/*','foo/**','foo/?','foo/[ab]','[a-z]','[!a-z]','[^a]','[-a]','[a-]','[!a]','{a,b}','{a,}','{,a}','{a,b}*','{foo/*,bar/**}','\\*','\\?','a\\.js','a,b','a}','[\\a]','[a&&b]','[a-c-e]','[[]','[\u00e9-\u0101]','[\u00e9]','\u00e9?','[\U0001f600]','{a,{b,c}}','{a','[','[]','[!]','[z-a]','[a/b]','[a-','\\','\u00e9[','\U0001f600[']
patterns += ['a[]', '[]x', '[]]', '[!]]', '[]a]', '[]*]', '{[],x}', '😀[]', '[a&b]', '[a&&]', '[a-\\]', '[\x00]', '[\x00-\x01]', '[😀-😁]', '[^]', '[--]', '[!]x]']
values=['','a','b','aa','a.js','foo/a','foo/b','foo/a.js','foo/bar/a.js','bar/a.js','/a','/foo/a.js','foo','foo/','foo//a','./a','../a','*','?','-','^','[','\\','\u00e9','\u0100','\U0001f600','\n','\r','\u0085','\u2028','\u2029','foo/\n']
cases=[{'pattern':p,'input':v} for p in patterns for v in values]
p=subprocess.run(['java','-Xmx2g','-cp',str(classes)+':'+str(jar),'CliGlob'],input=json.dumps(cases).encode(),stdout=subprocess.PIPE,check=True,env={**os.environ,'LANG':'C.UTF-8'})
(cache/'glob-raw.json').write_bytes(p.stdout);rows=json.loads(p.stdout)
(root/'tests/data/glob_golden.json').write_text(json.dumps(rows,ensure_ascii=True,indent=2)+'\n');print(len(rows),'glob cases')
