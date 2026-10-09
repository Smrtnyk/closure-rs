"""Capture the CLI's Gson reader, UTF-16 values, and pretty output bytes."""
import base64,json,os,subprocess
from pathlib import Path
import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
import paths as _paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout
repo=Path(_paths.ROOT);root=Path(__file__).resolve().parents[1];cache=repo/'corpus-cache/cli';classes=cache/'classes';jar=Path(_paths.REF_JAR)
subprocess.run(['javac','-cp',str(jar),'-d',str(classes),str(root/'tools/CliJson.java')],check=True)
cases=['[]','[{}]','[null]','[null,{}]','[{"src":"x","path":"a.js","source_map":"map","webpack_id":"0"}]']
for v in ['null','true','false','TRUE','FALSE','1','-0','1e3','1.0','01','0x1','NaN','Infinity','"x"',"'x'",'unquoted','"\\ud800"','"\\udc00"','"\\ud83d\\ude00"','"\\u2028\\u2029<>&=\\t\\n"','"a\\q"','"\\uabcd"','"\\uZZZZ"','[]','{}']:
 for field in ['src','path','sourceMap','source_map','webpackId','webpack_id','unknown']:
  cases.append('[{"'+field+'":'+v+'}]')
cases += ['','null','{}','[','[{}','[{},]','[,{}]','[{};{}]','[{src:x,path:"a"}]','[/*x*/{}]','[{/*x*/src:"x"}]','[{"src"="x"}]','[{"src"=>"x"}]','[{"src":"x",}]','[{"src":"x";"path":"a"}]',"[{'src':'x','path':'a'}]",'[{"sourceMap":"a","source_map":"b"}]','[{"src":[{}]}]','[{}] trailing','\ufeff[{}]', ")]}'\n[{}]", '[true]','[1]','["x"]']
p=subprocess.run(['java','-Xmx2g','-cp',str(classes)+':'+str(jar),'com.google.javascript.jscomp.CliJson'],input=json.dumps(cases).encode(),stdout=subprocess.PIPE,check=True,env={**os.environ,'LANG':'C.UTF-8'})
(cache/'json-raw.json').write_bytes(p.stdout);rows=json.loads(p.stdout)
for row in rows:
 r=row['result']
 if 'output' in r:r['output']=list(base64.b64decode(r['output']))
(root/'tests/data/json_golden.json').write_text(json.dumps(rows,ensure_ascii=True,indent=2)+'\n');print(len(rows),'JSON cases')
