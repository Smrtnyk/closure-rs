# Benchmark results

`scripts/run_bench.py` writes one JSON file per run here, named `<UTC date>-<commit>.json`
(override with `--out`). The files are git-ignored: results depend on the machine, its load
and the commit, so they are not committed. Only this README is tracked.

## Format (schema 1)

```
{
  "schema": 1,
  "date_utc": "YYYY-MM-DDTHH:MM:SSZ",
  "commit": "<git HEAD of the checkout, short>",
  "reps": N,
  "host": {"machine", "kernel", "cpus", "loadavg_start": [1, 5, 15 min], "loadavg_end", "nice"},
  "java": {"java": path, "jar": path, "jar_sha256", "version"},
  "rust": {"bin": path, "bin_sha256"},
  "jobs": [
    {
      "id": "d3-12/d3-array/SIMPLE",          // <project>/<job>/<level> (<job>/<level> if equal)
      "project", "job", "level",
      "inputs": number of --js files, "input_bytes": their total size,
      "args": [the compiler argv after `java -jar <jar>` or `closure-rs`],
      "java" | "rust": {
        "wall_s": median wall-clock seconds, "rss_kb": median peak RSS (KiB),
        "cpu_s": median user+system CPU seconds,
        "exit", "timed_out", "output_bytes", "stderr_bytes",   // of the first repetition
        "reps": [{"wall_s", "rss_kb", "user_s", "sys_s", "exit"}, ...],
        "nondeterministic_reps": [i, ...]      // only if repetition i differs from the first
      },
      "identical": true | false,               // every Rust run == Java's first run, byte for byte
      "diff": null | {"exit" | "stdout" | "stderr" | "output" | "extra_files": details}
                                               // first differing byte: offset, line, col,
                                               // both lengths, context from both sides
    }, ...
  ],
  "totals": {
    "<project>/<level>" | "ALL/<level>": {
      "jobs": n, "java" | "rust": {"wall_s": sum of the medians, "max_rss_kb": max of the medians},
      "identical": number of identical jobs
    }
  }
}
```

`--impl java` or `--impl rust` runs one compiler only; the other's keys and `identical` are then
absent. The Markdown table printed to stdout shows the same medians and the Rust/Java ratios
(D7 holds for a job when both ratios are at most 1.00).
