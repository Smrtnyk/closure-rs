"""Minimal Python client for the closure-rs oracle server (oracle/PROTOCOL.md).

    from oracle_client import Oracle
    with Oracle() as o:
        r = o.request({"op": "compile", "args": [...]})
"""
import base64
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
from paths import ORACLE_JAR, REF_JAR  # noqa: E402  the reference's jars (scripts/paths.py)
JAVA = os.path.join(ROOT, "tools", "jdk-21", "bin", "java")
CP = ORACLE_JAR + ":" + REF_JAR


def golden_env() -> dict:
    """The process environment of every golden run and of Gate 0.1 (PROTOCOL.md, "Environment").

    Identical to gates/lib/run_reference.child_env(): nothing inherited, LANG=LC_ALL=C.UTF-8,
    TZ=UTC. The compiler's output depends on it (the summary is formatted with the default
    locale, "%.1f%% typed"; stdout/stderr are encoded with stdout.encoding/stderr.encoding).
    """
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": os.environ.get("HOME", "/tmp"),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "TZ": "UTC",
    }


# What the JDK 21 oracle JVM derives from golden_env(), as reported in the server's ready line
# ("env", Main.jvmEnv()). Measured 2026-10-06 with tools/jdk-21 (Temurin 21.0.12.1+1).
GOLDEN_JVM_ENV = {
    "user.language": "en",
    "user.country": None,
    "user.variant": None,
    "locale.default": "en",
    "locale.format": "en",
    "native.encoding": "UTF-8",
    "stdout.encoding": "UTF-8",
    "stderr.encoding": "UTF-8",
    "sun.jnu.encoding": "UTF-8",
    "file.encoding": "UTF-8",
    "timezone": "UTC",
}


class OracleEnvMismatch(RuntimeError):
    pass


def check_jvm_env(env: dict | None) -> None:
    """Raise unless the oracle JVM runs in the golden environment."""
    if not isinstance(env, dict):
        raise OracleEnvMismatch(f"oracle ready line has no env (old oracle.jar?): {env!r}")
    bad = {k: (env.get(k), v) for k, v in GOLDEN_JVM_ENV.items() if env.get(k) != v}
    if bad:
        raise OracleEnvMismatch(
            "oracle JVM environment differs from the golden one (got, want): " + repr(bad))


class Oracle:
    """One oracle server. By default it runs in golden_env() and refuses to start if the
    JVM-derived locale, encodings or time zone differ from GOLDEN_JVM_ENV. Pass env=... and
    check_env=False only for experiments whose results are not ground truth."""

    def __init__(self, isolate="none", xmx="2g", extra_jvm=(), env=None, check_env=True):
        self.proc = subprocess.Popen(
            [JAVA, f"-Xmx{xmx}", *extra_jvm, "-cp", CP, "closurers.oracle.Main", "server",
             f"--isolate={isolate}"],
            cwd=ROOT, env=golden_env() if env is None else env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, bufsize=0)
        hello = json.loads(self.proc.stdout.readline())
        assert hello.get("ready"), hello
        self.hello = hello
        self.env = hello.get("env")
        if check_env:
            try:
                check_jvm_env(self.env)
            except Exception:
                self.close()
                raise
        self.n = 0

    def request(self, req):
        self.n += 1
        req = dict(req)
        req.setdefault("id", self.n)
        self.proc.stdin.write((json.dumps(req) + "\n").encode())
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("oracle server died")
        return json.loads(line)

    def close(self):
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=30)
        except Exception:
            self.proc.kill()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        self.close()


def b64(s):
    return base64.b64decode(s) if s is not None else None


def cli_compile(args, timeout=600, env=None):
    """One fresh JVM per request: `Main compile ARGV` (behaves like java -jar), in golden_env().

    The CLI mode prints nothing but the compiler's own output, so it cannot report its
    environment; the pinned golden_env() is what makes it ground truth."""
    p = subprocess.run([JAVA, "-Xmx2g", "-cp", CP, "closurers.oracle.Main", "compile", *args],
                       cwd=ROOT, env=golden_env() if env is None else env,
                       stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
    return p.returncode, p.stdout, p.stderr
