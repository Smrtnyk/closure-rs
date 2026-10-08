"""Gate 0.2 sub-check: test methods that assert after a hooked call (gate 0.2 (a)).

Static audit scan (it extracts nothing; records still come only from the runtime hook): for every
record-bearing @Test method of the pristine test sources, find assert*/verify*/checkState/
checkNotNull statements after the first hooked call in the method body (excluding postcondition(...)
and assertThrows(...) bodies, and helpers that are themselves hooked), directly or through a
non-@Test helper that does so. A call to a non-@Test helper that reaches a hooked
call at any depth (testAndReturnAsts, testScoped, compileToTypes, ...) is itself a hook position.
Every flagged method must have its post-call state checked by replay or be
classified out of scope in its descriptor (`postCallAssertions`), else gate (a) fails.
"""
import collections
import gzip
import json
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "scripts"))
from paths import ROOT  # noqa: E402  the main checkout (scripts/paths.py)
PRISTINE_TEST = ROOT + "/reference/closure-compiler/test/"

HOOK = (r'\b(test|testSame|testError|testWarning|testNoWarning|testErrorAndWarning|testExternChanges|testTypes|'
        r'testTypesWithExterns|testTypesWithCommonExterns|testClosureTypes|testClosureTypesMultipleWarnings|newTest|'
        r'testParseError|testNoWarnings|compile|parseAndTypeCheckWithScope|parseAndTypeCheck|testSets|testUnusedLocals|'
        r'assertNoWarning|testWarnings|testSameWithInversion|foldSame|fold|testSameEs6|testDefineOverrides|'
        r'testDefaultInterop|testTranspiled|testWithPrefix)\s*\(')
ASSERT = r'\b(assert(?!Throws\b)\w*|verify\w*|checkState|checkNotNull)\s*\('
# Broader assertion pattern (check[A-Z]*, expect* except expected*, fail(). It is
# applied only after the END of the first hooked call, so arguments of the hooked call (for
# example expectExterns(...) postconditions, which run inside the call and are covered by the
# postcondition-count rule) are not taken for post-call assertions.
ASSERT_BROAD = r'\b(assert(?!Throws\b)\w*|verify\w*|check[A-Z]\w*|checkState|checkNotNull|expect(?!ed)\w*|fail)\s*\('


RECFILE = {}  # record class FQCN -> record file stem (nested classes live in the outer class's file)


class UnresolvedSource(Exception):
    pass


def source_path(fq):
    """Pristine source file of record class fq; a nested class Foo$Bar resolves to Foo.java.
    Raises UnresolvedSource when no file exists (gate (a) then fails loudly)."""
    top = fq.split('$', 1)[0]
    path = PRISTINE_TEST + top.replace('.', '/') + '.java'
    if not os.path.exists(path):
        raise UnresolvedSource(f"{fq}: no source file {path}")
    return path


def _strip(src):
    """Blanks comments, strings, text blocks and char literals (newlines kept)."""
    out, i, n = [], 0, len(src)
    while i < n:
        if src.startswith('"""', i):
            j = src.find('"""', i + 3)
            j = n if j < 0 else j + 3
            out.append(''.join('\n' if c == '\n' else ' ' for c in src[i:j]).replace(' ', 'S', 1))
            i = j
            continue
        c = src[i]
        if src.startswith('//', i):
            j = src.find('\n', i)
            j = n if j < 0 else j
            out.append(' ' * (j - i))
            i = j
            continue
        if src.startswith('/*', i):
            j = src.find('*/', i + 2)
            j = n if j < 0 else j + 2
            out.append(''.join('\n' if ch == '\n' else ' ' for ch in src[i:j]))
            i = j
            continue
        if c in '"\'':
            j = i + 1
            while j < n and src[j] != c:
                j += 2 if src[j] == '\\' else 1
            j += 1
            out.append('S' + ' ' * (j - i - 1))
            i = j
            continue
        out.append(c)
        i += 1
    return ''.join(out)


def _methods(s):
    for m in re.finditer(r'(@Test\b[^{;]*?)?\b(?:public|private|protected|static|final|void|\s)+[\w<>\[\],. ?]*?\b(\w+)\s*\([^)]*\)\s*(?:throws [\w., ]+)?\{', s):
        name = m.group(2)
        if name in ('if', 'for', 'while', 'switch', 'catch', 'synchronized'):
            continue
        i, d = m.end(), 1
        while i < len(s) and d:
            if s[i] == '{':
                d += 1
            elif s[i] == '}':
                d -= 1
            i += 1
        yield name, bool(m.group(1)), m.end(), i


def _spans(s, b, e, kw):
    spans = []
    for m in re.finditer(r'\b' + kw + r'\s*\(', s[b:e]):
        i, d = b + m.end(), 1
        while i < e and d:
            if s[i] == '(':
                d += 1
            elif s[i] == ')':
                d -= 1
            i += 1
        spans.append((b + m.start(), i))
    return spans


def _call_end(s, start, e):
    """End offset of the call whose name starts at `start` (matching parentheses)."""
    i = s.index('(', start) + 1
    d = 1
    while i < e and d:
        if s[i] == '(':
            d += 1
        elif s[i] == ')':
            d -= 1
        i += 1
    return i


def self_test(flagged):
    """Regression: these methods assert after the hooked call and must be flagged."""
    must = [("com.google.javascript.jscomp.DevirtualizeMethodsTest", "testRewritePrototypeMethodsWithCorrectColors"),
            # assertion that encloses the hooked call
            ("com.google.javascript.jscomp.serialization.SerializeTypedAstPassTest", "testAst_externs"),
            # hooked call reached through a non-@Test helper whose name is not in HOOK
            ("com.google.javascript.jscomp.Es6RewriteClassTest", "testSimpleClassStatement_hasCorrectSourceInfo"),
            ("com.google.javascript.jscomp.ScopedAliasesTest", "testSourceInfo"),
            ("com.google.javascript.jscomp.serialization.JSTypeReconserializerTest", "testNativeTypesAreNotSerialized")]
    missing = [f"{c}#{m}" for c, m in must if m not in flagged.get(c, {})]
    if missing:
        raise AssertionError("unit_postassert_scan self-test: not flagged: " + ", ".join(missing))


def scan(records_dir):
    """Returns {class FQCN: {method: number of direct post-call assertion statements (or -1 via helper)}}."""
    recmeth = collections.defaultdict(collections.Counter)
    # D-017 item 7: methods whose recorded calls ran at least one postcondition (expected.postconditions).
    postcond = collections.defaultdict(set)
    for fn in sorted(os.listdir(records_dir)):
        if not fn.endswith('.jsonl.gz'):
            continue
        with gzip.open(os.path.join(records_dir, fn), 'rt', encoding='utf-8') as fh:
            for line in fh:
                r = json.loads(line)
                recmeth[r['class']][r['method'].split('[')[0]] += 1
                if ((r.get('expected') or {}).get('postconditions') or 0) > 0:
                    postcond[r['class']].add(r['method'].split('[')[0])
                RECFILE[r['class']] = fn[:-len('.jsonl.gz')]
    flagged, unresolved = {}, []
    for fq in sorted(recmeth):
        try:
            path = source_path(fq)
        except UnresolvedSource as e:
            unresolved.append(str(e))
            continue
        with open(path, encoding='utf-8') as f:
            s = _strip(f.read())
        ms = list(_methods(s))
        bodies = {}
        for nm, ist, bb, ee in ms:
            if not ist:
                bodies.setdefault(nm, []).append(s[bb:ee])
        # A non-@Test helper that reaches a hooked call, directly or
        # through other helpers at any depth (testAndReturnAsts, testScoped, compileToTypes, ...), is
        # itself a hooked call. Its call sites in @Test bodies are hook positions for both the
        # "after the call" and the "encloses the call" rules below.
        helpers = {nm for nm, bs in bodies.items() if any(re.search(HOOK, x) for x in bs)}
        changed = True
        while changed:
            changed = False
            for nm, bs in bodies.items():
                if nm in helpers:
                    continue
                if any(re.search(r'\b' + re.escape(h) + r'\s*\(', x) for h in helpers for x in bs):
                    helpers.add(nm)
                    changed = True
        hooknames = {nm for nm, _, bb, ee in ms if re.search(HOOK, s[bb:ee])} | helpers
        hook_re = HOOK if not helpers else (
            r'(?:' + HOOK + r'|\b(?:' + '|'.join(sorted(map(re.escape, helpers))) + r')\s*\()')
        # Transitive closure: a non-@Test, non-hook method "asserts" when its body contains an
        # assertion call anywhere, or calls (at any depth) another method that asserts.
        asserting = {nm for nm, bs in bodies.items()
                     if nm not in hooknames and any(re.search(ASSERT_BROAD, x) for x in bs)}
        changed = True
        while changed:
            changed = False
            for nm, bs in bodies.items():
                if nm in asserting or nm in hooknames:
                    continue
                if any(re.search(r'\b' + re.escape(h) + r'\s*\(', x) for h in asserting for x in bs):
                    asserting.add(nm)
                    changed = True
        info, helper_post = {}, set()
        for name, ist, b, e in ms:
            body = s[b:e]
            hooks = [m.start() + b for m in re.finditer(hook_re, body)]
            if not hooks:
                info[name] = (ist, [])
                continue
            hook_end = _call_end(s, hooks[0], e)
            excl = _spans(s, b, e, 'postcondition') + _spans(s, b, e, 'assertThrows')
            # D-017 item 7: an assertion inside a lambda passed to a hooked call (a Postcondition
            # handed to the harness, e.g. `(Postcondition) compiler -> {...}` as an argument of a
            # second test(...) call) runs inside that call: it is an in-call assertion, governed by
            # the postcondition-count rule and gate (e), not by post-call classification. Such an
            # assertion lies inside the hooked call's argument list, after a `->` that starts there;
            # the rule applies only to methods whose recorded calls ran postconditions
            # (expected.postconditions > 0), so other in-call lambdas (e.g. ReferenceCollectorTest's
            # Behavior callbacks) stay under post-call classification.
            lam = []
            # (a non-@Test helper qualifies when its class has such methods: it is where the
            # Postcondition lambda is built, e.g. ExternExportsPassTest.compileAndCheck)
            for h in (hooks if name in postcond[fq] or (not ist and postcond[fq]) else []):
                he = _call_end(s, h, e)
                k = s.find('->', h, he)
                if k >= 0:
                    lam.append((k, he))
            asserts = []
            for m in re.finditer(ASSERT, body):
                p = b + m.start()
                if m.group(1) in hooknames or any(x <= p < y for x, y in excl) or any(x <= p < y for x, y in lam):
                    continue
                if p <= hooks[0]:
                    # An assertion whose argument list contains a hooked call
                    # (`assertThat(compile(...)).hasSize(2)`) checks that call's result after it
                    # returns, so it is a post-call assertion too.
                    end = _call_end(s, p, e)
                    if any(p < h < end for h in hooks):
                        asserts.append(p)
                    continue
                asserts.append(p)
            for m in re.finditer(ASSERT_BROAD, body):
                p = b + m.start()
                if (m.group(1) in hooknames or p < hook_end or any(x <= p < y for x, y in excl)
                        or any(x <= p < y for x, y in lam) or p in asserts):
                    continue
                asserts.append(p)
            for h in asserting:
                if h == name:
                    continue
                for m in re.finditer(r'\b' + re.escape(h) + r'\s*\(', body):
                    p = b + m.start()
                    if p >= hook_end and not any(x <= p < y for x, y in excl) and p not in asserts:
                        asserts.append(p)
            info[name] = (ist, asserts)
            if asserts and not ist:
                helper_post.add(name)
        # helpers that assert after their own hooked call, closed transitively over callers
        changed = True
        while changed:
            changed = False
            for nm, bs in bodies.items():
                if nm in helper_post:
                    continue
                if any(re.search(r'\b' + re.escape(h) + r'\s*\(', x) for h in helper_post for x in bs):
                    helper_post.add(nm)
                    changed = True
        for name, ist, b, e in ms:
            if not ist or recmeth[fq].get(name, 0) == 0:
                continue
            asserts = info.get(name, (True, []))[1]
            via = [h for h in helper_post if re.search(r'\b' + re.escape(h) + r'\s*\(', s[b:e])]
            if asserts or via:
                flagged.setdefault(fq, {})[name] = {"directAsserts": len(asserts), "viaHelpers": sorted(via)[:5]}
    if unresolved:
        raise UnresolvedSource("record classes without a resolvable source file: " + "; ".join(unresolved))
    return flagged


if __name__ == "__main__":
    import sys
    f = scan(sys.argv[1] if len(sys.argv) > 1 else ROOT + "/corpus/unit/records")
    self_test(f)
    print("self-test ok;", sum(len(v) for v in f.values()), "methods in", len(f), "classes")
