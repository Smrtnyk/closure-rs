#!/usr/bin/env bash
# Oracle smoke test: exercises every operation through the CLI and through the server.
# Usage: oracle/test/smoke.sh        (builds the oracle first if oracle.jar is missing)
# The jars are the reference's ($ORACLE_JAR, $REF_JAR; scripts/paths.sh, docs/PORTING.md §9).
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/../../scripts/paths.sh"  # ROOT, REF_JAR, ORACLE_JAR
# ROOT is the main checkout (tools/, corpus-cache/); HERE is this checkout (oracle/ sources).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$HERE"
. "$ROOT/tools/env.sh"
[ -f "$ORACLE_JAR" ] || oracle/build.sh
JAR="$REF_JAR"
CP="$ORACLE_JAR:$JAR"
# Every JVM runs in the golden environment (PROTOCOL.md "Environment" = run_reference.child_env()).
JAVA_BIN="$ROOT/tools/jdk-21/bin/java"
GENV=(env -i PATH=/usr/bin:/bin HOME="$HOME" LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC)
T="$(dirname "$ORACLE_JAR")/tmp/smoke"
rm -rf "$T"; mkdir -p "$T"
fail=0
ok()  { echo "PASS $*"; }
bad() { echo "FAIL $*"; fail=1; }

cat > "$T/a.js" <<'EOF'
/** @const */ var ns = {};
/** @param {number} x @return {number} */
ns.sq = function(x) { var unused = 1; return x * x; };
window['out'] = ns.sq(7) + `t${1}`.length;
EOF

# 1. compile via CLI == java -jar (stdout, stderr, exit code), for a success and a failure case.
for argv in "--compilation_level=ADVANCED --js=$T/a.js" \
            "--compilation_level=SIMPLE --js=$T/a.js --formatting=PRETTY_PRINT" \
            "--js=$T/does-not-exist.js" "--bogus_flag"; do
  set +e
  "${GENV[@]}" "$JAVA_BIN" -jar "$JAR" $argv < /dev/null > "$T/j.out" 2> "$T/j.err"; jrc=$?
  "${GENV[@]}" "$JAVA_BIN" -cp "$CP" closurers.oracle.Main compile $argv < /dev/null > "$T/o.out" 2> "$T/o.err"; orc=$?
  set -e
  if [ $jrc = $orc ] && cmp -s "$T/j.out" "$T/o.out" && cmp -s "$T/j.err" "$T/o.err"; then
    ok "cli compile == java -jar (rc=$jrc): $argv"
  else
    bad "cli compile != java -jar (rc $jrc vs $orc): $argv"
  fi
done

# 2. every operation through the server (both isolation modes).
python3 - "$T" "$ROOT" <<'EOF' || fail=1
import base64, json, subprocess, sys
sys.path.insert(0, "oracle")
from oracle_client import Oracle, b64, JAVA, golden_env, REF_JAR
T, MAIN_ROOT = sys.argv[1], sys.argv[2]
A = ["--compilation_level=ADVANCED", f"--js={T}/a.js"]
jar = subprocess.run([JAVA, "-jar", REF_JAR, *A],
                     env=golden_env(), stdin=subprocess.DEVNULL, capture_output=True)
fails = 0
def check(name, cond, info=""):
    global fails
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else f"  {info}"))
    fails += 0 if cond else 1
for iso in ("none", "request"):
    with Oracle(isolate=iso, xmx="768m") as o:
        r = o.request({"op": "compile", "args": A})
        check(f"[{iso}] compile == java -jar", r["ok"] and b64(r["stdout_b64"]) == jar.stdout
              and b64(r["stderr_b64"]) == jar.stderr and r["exit_code"] == jar.returncode, r)
        r = o.request({"op": "compile", "args": A + [f"--js_output_file={T}/out.js",
                       f"--create_source_map={T}/out.js.map"]})
        check(f"[{iso}] compile captures output files", r["ok"] and
              set(r["output_files"]) == {f"{T}/out.js", f"{T}/out.js.map"}, r.get("output_files", {}).keys())
        r = o.request({"op": "compile", "args": ["--js=x.js", "--js_output_file=o.js"],
                       "files": {"x.js": "var a = 1; alert(a);"}})
        check(f"[{iso}] compile inline files", r["ok"] and
              base64.b64decode(r["output_files"]["o.js"]) == b"var a=1;alert(a);\n", r)
        r = o.request({"op": "compile_with_pass_dumps", "args": A})
        names = [p["pass"] for p in r.get("passes", [])]
        check(f"[{iso}] compile_with_pass_dumps", r["ok"] and "parseInputs" in names and
              r["passes"][-1]["source"] + "\n" == b64(r["stdout_b64"]).decode(), names)
        r = o.request({"op": "checks_to_typedast", "args": A})
        td = r.get("typedast_b64")
        check(f"[{iso}] checks_to_typedast", r["ok"] and td and base64.b64decode(td)[:2] == b"\x1f\x8b", r.get("error"))
        r2 = o.request({"op": "optimize_from_typedast", "args": A + ["--dependency_mode=NONE"],
                        "typedast_b64": td})
        check(f"[{iso}] optimize_from_typedast == compile", r2["ok"] and r2["exit_code"] == 0 and
              b64(r2["stdout_b64"]) == jar.stdout, (r2.get("error") or b64(r2.get("stderr_b64") or "")))
        # parse_dump: deep AST (no depth limit), non-LF line terminators ($threw marker),
        # sourceMappingURL, raw report stream with --jscomp_error=*.
        deep = "var d = " + "[" * 5000 + "]" * 5000 + ";\n"
        r = o.request({"op": "parse_dump", "content": deep})
        check(f"[{iso}] parse_dump 5000-deep AST", r.get("ok") and not r["errors"], r.get("error", "")[:300])
        for f in ("line-terminators/comment-multi-cr.js", "line-terminators/between-tokens-cr.js",
                  "expressions/template-literal/tv-line-continuation.js",
                  "expressions/logical-assignment/lgcl-and-whitespace.js"):
            path = MAIN_ROOT + "/corpus-cache/d2/test262/test/language/" + f
            import os
            if not os.path.exists(path):
                print("SKIP parse_dump " + f + " (corpus cache missing)"); continue
            r = o.request({"op": "parse_dump", "path": path, "language_in": "UNSTABLE"})
            check(f"[{iso}] parse_dump non-LF terminators {f}", r.get("ok"), r.get("error", "")[:300])
        r = o.request({"op": "parse_dump", "content": "var a=1;\n//# sourceMappingURL=foo.js.map \n"})
        check(f"[{iso}] parse_dump source_map_url", r.get("ok") and r["source_map_url"] == "foo.js.map", r.get("source_map_url"))
        r = o.request({"op": "parse_dump", "content": "var s = '\\q';\n", "args": ["--jscomp_error=*"]})
        check(f"[{iso}] parse_dump guards applied", r.get("ok") and [e["key"] for e in r["errors"]] == ["JSC_UNNECESSARY_ESCAPE"]
              and r["reports"][0]["effective_level"] == "ERROR", r.get("reports"))
        r = o.request({"op": "compile_with_pass_dumps", "args": A + [f"--js_output_file={T}/pd.js",
                       f"--create_source_map={T}/pd.js.map"]})
        check(f"[{iso}] compile_with_pass_dumps strips --create_source_map", r.get("ok") and r["exit_code"] == 0
              and r["stripped_args"] == [f"--create_source_map={T}/pd.js.map"] and len(r["passes"]) > 0, r.get("stripped_args"))
        r = o.request({"op": "parse_dump", "path": f"{T}/a.js", "language_in": "ECMASCRIPT_2020"})
        ok_ = r["ok"] and r["ast"]["token"] == "SCRIPT" and not r["errors"]
        asg = r["ast"]["children"][1]["children"][0] if ok_ else {}
        check(f"[{iso}] parse_dump", ok_ and asg.get("token") == "ASSIGN" and "JSDOC_INFO" in asg["props"]
              and asg["children"][1]["token"] == "FUNCTION"
              and asg["props"]["JSDOC_INFO"]["getParameterNames"] == ["x"], r.get("error"))
        r = o.request({"op": "parse_dump", "content": "var x = ;", "language_in": "ECMASCRIPT_2020"})
        check(f"[{iso}] parse_dump reports parse errors", r["ok"] and len(r["errors"]) == 1, r)
sys.exit(1 if fails else 0)
EOF

# 3. one-shot request mode
echo '{"op":"parse_dump","content":"1n + 0x10","language_in":"ECMASCRIPT_2020"}' \
  | "${GENV[@]}" "$JAVA_BIN" -Xmx512m -cp "$CP" closurers.oracle.Main request > "$T/req.json"
python3 -c "import json,sys; d=json.load(open('$T/req.json')); sys.exit(0 if d['ok'] and d['ast']['children'][0]['children'][0]['children'][1]['double_bits']=='0x4030000000000000' else 1)" \
  && ok "request mode parse_dump number bits" || bad "request mode parse_dump"

[ $fail = 0 ] && echo "SMOKE OK" || { echo "SMOKE FAILED"; exit 1; }
