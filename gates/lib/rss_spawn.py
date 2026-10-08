"""Tiny exec wrapper that measures a child's true peak RSS.

Usage: python3 -I -S gates/lib/rss_spawn.py <rss-out-file> -- <argv...>

Why: Linux records the *old* mm's high-water RSS into the process's maxrss at execve().
CPython's subprocess spawns with vfork (shared parent mm), so a child started directly
from a large driver process reports max(driver peak RSS, child peak RSS) in wait4().
This wrapper is a small fresh process: it fork()s (from ~10 MB, not the driver's size),
execs argv unchanged (same env, cwd, stdio; SIGPIPE/SIGXFSZ restored to default like
subprocess' restore_signals), waits, writes the child's ru_maxrss (kB) to <rss-out-file>,
and exits with the child's status (re-raising the child's terminating signal).
"""
import os
import signal
import sys


def main() -> None:
    rss_file = sys.argv[1]
    assert sys.argv[2] == "--"
    argv = sys.argv[3:]
    pid = os.fork()
    if pid == 0:
        for s in ("SIGPIPE", "SIGXFZ", "SIGXFSZ"):
            if hasattr(signal, s):
                signal.signal(getattr(signal, s), signal.SIG_DFL)
        try:
            os.execv(argv[0], argv)
        finally:
            os._exit(127)
    _pid, status, ru = os.wait4(pid, 0)
    with open(rss_file, "w") as f:
        f.write(f"{ru.ru_maxrss}\n")
    if os.WIFSIGNALED(status):
        sig = os.WTERMSIG(status)
        signal.signal(sig, signal.SIG_DFL)
        os.kill(os.getpid(), sig)
    sys.exit(os.waitstatus_to_exitcode(status))


if __name__ == "__main__":
    main()
