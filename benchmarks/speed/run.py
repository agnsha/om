"""Runtime speed: Om vs Rust vs Python on the same seven programs.

Usage: python3 run.py [--runs 5] [--quick] [--out results.json]

Om is built with `om build` (Cranelift, the only backend today). Rust is built
with `rustc -C opt-level=3` (its release default) and again with
`-C overflow-checks=on`, because Om checks integer overflow in every build.
Python is CPython. Every program prints a result, and the run fails unless all
languages print the same thing.
"""

import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))

# name -> (argument for the full run, argument for --quick)
PROGRAMS = {
    "fib": ("38", "25"),
    "sieve": ("20000000", "100000"),
    "collatz": ("500000", "10000"),
    "matmul": ("400", "40"),
    "sort": ("5000000", "10000"),
    "strings": ("5000000", "10000"),
    "nbody": ("500000", "1000"),
}


def om_binary():
    for p in (os.environ.get("OM"), os.path.join(REPO, "target", "release", "om"), shutil.which("om")):
        if p and os.path.exists(p):
            return p
    sys.exit("om not found: run `cargo build --release -p omc` or set OM=/path/to/om")


def timed(cmd, **kw):
    t = time.perf_counter()
    out = subprocess.run(cmd, capture_output=True, text=True, **kw)
    dt = time.perf_counter() - t
    if out.returncode != 0:
        sys.exit(f"failed: {' '.join(cmd)}\n{out.stderr}")
    return dt, out.stdout.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5)
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--out", default=os.path.join(HERE, "results.json"))
    a = ap.parse_args()
    om = om_binary()
    tmp = tempfile.mkdtemp()
    ext = ".exe" if os.name == "nt" else ""
    results = []
    for name, (full, quick) in PROGRAMS.items():
        arg = quick if a.quick else full
        exe = {k: os.path.join(tmp, f"{name}-{k}{ext}") for k in ("om", "rs", "rs_checked")}
        build = {}
        build["om"], _ = timed([om, "build", os.path.join(HERE, f"{name}.om"), "-o", exe["om"]])
        build["rs"], _ = timed(["rustc", "-C", "opt-level=3", "-o", exe["rs"], os.path.join(HERE, f"{name}.rs")])
        build["rs_checked"], _ = timed(["rustc", "-C", "opt-level=3", "-C", "overflow-checks=on", "-o", exe["rs_checked"], os.path.join(HERE, f"{name}.rs")])
        cmds = {
            "om": [exe["om"], arg],
            "rs": [exe["rs"], arg],
            "rs_checked": [exe["rs_checked"], arg],
            "py": [sys.executable, os.path.join(HERE, f"{name}.py"), arg],
        }
        times, outputs = {}, {}
        for lang, cmd in cmds.items():
            runs = 1 if lang == "py" and not a.quick else a.runs
            ts = []
            for _ in range(runs):
                dt, out = timed(cmd)
                ts.append(dt)
                outputs[lang] = out
            times[lang] = statistics.median(ts)
        if len(set(outputs.values())) != 1:
            sys.exit(f"{name}: outputs differ: {outputs}")
        row = {"program": name, "arg": arg, "output": outputs["om"],
               "run_s": times, "build_s": build}
        results.append(row)
        print(f"{name:8} om {times['om']:.3f}s  rust {times['rs']:.3f}s  rust+checks {times['rs_checked']:.3f}s  "
              f"python {times['py']:.3f}s  | build om {build['om']:.2f}s rust {build['rs']:.2f}s", flush=True)
    meta = {
        "machine": platform.processor() or platform.machine(),
        "os": platform.platform(),
        "python": platform.python_version(),
        "rustc": subprocess.run(["rustc", "--version"], capture_output=True, text=True).stdout.strip(),
        "om": subprocess.run([om, "--version"], capture_output=True, text=True).stdout.strip(),
        "runs": a.runs,
        "quick": a.quick,
    }
    with open(a.out, "w") as f:
        json.dump({"meta": meta, "results": results}, f, indent=2)
    print(f"wrote {a.out}")


if __name__ == "__main__":
    main()
