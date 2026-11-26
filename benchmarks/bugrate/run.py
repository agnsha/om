"""Bug rate: compile and run the programs a model wrote for each task.

Usage: python3 run.py            # every responses/<model>/<lang>.md

A response is the model's raw reply to prompts/<lang>.md: a `### <task>` line
and one fenced code block per task. Each program is extracted to
attempts/<model>/<lang>/<task>.<ext>, built (`om build`, `rustc -O`, or
`python3 -m py_compile`), run, and its output compared with
tasks/<task>.expected. One attempt, no compiler feedback (pass@1).

Outcomes: pass, compile (did not build), runtime (crashed or timed out),
wrong (ran but printed something else), missing (no program in the reply).
"""

import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
EXT = {"om": "om", "rust": "rs", "python": "py"}
TASKS = sorted(os.path.basename(p)[:-9] for p in glob.glob(os.path.join(HERE, "tasks", "*.expected")))


def om_binary():
    for p in (os.environ.get("OM"), os.path.join(REPO, "target", "release", "om"), shutil.which("om")):
        if p and os.path.exists(p):
            return p
    sys.exit("om not found: run `cargo build --release -p omc` or set OM=/path/to/om")


def extract(text):
    progs = {}
    for m in re.finditer(r"^###\s*(\S+)\s*\n+```[a-zA-Z]*\n(.*?)\n```", text, re.S | re.M):
        progs[m.group(1).strip("`")] = m.group(2) + "\n"
    return progs


def run(cmd, timeout=20):
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "timeout"


def check(lang, src, exe, om):
    if lang == "om":
        code, _, err = run([om, "build", src, "-o", exe])
        cmd = [exe]
    elif lang == "rust":
        code, _, err = run(["rustc", "--edition", "2021", "-O", "-o", exe, src], timeout=120)
        cmd = [exe]
    else:
        code, _, err = run([sys.executable, "-m", "py_compile", src])
        cmd = [sys.executable, src]
    if code != 0:
        return "compile", err.strip()
    code, out, err = run(cmd)
    if code != 0:
        return "runtime", (err or out).strip()
    return None, out


def main():
    om = om_binary()
    tmp = tempfile.mkdtemp()
    report = {}
    for resp in sorted(glob.glob(os.path.join(HERE, "responses", "*", "*.md"))):
        model = os.path.basename(os.path.dirname(resp))
        lang = os.path.basename(resp)[:-3]
        progs = extract(open(resp, encoding="utf-8").read())
        outdir = os.path.join(HERE, "attempts", model, lang)
        os.makedirs(outdir, exist_ok=True)
        rows = {}
        for task in TASKS:
            if task not in progs:
                rows[task] = {"outcome": "missing"}
                continue
            src = os.path.join(outdir, f"{task}.{EXT[lang]}")
            with open(src, "w", encoding="utf-8") as f:
                f.write(progs[task])
            outcome, detail = check(lang, src, os.path.join(tmp, f"{model}-{lang}-{task}"), om)
            if outcome is None:
                want = open(os.path.join(HERE, "tasks", task + ".expected")).read()
                outcome = "pass" if detail.strip() == want.strip() else "wrong"
                detail = "" if outcome == "pass" else detail
            rows[task] = {"outcome": outcome, "detail": detail.replace(HERE + os.sep, "")[:600]}
        report.setdefault(model, {})[lang] = rows
        n = sum(r["outcome"] == "pass" for r in rows.values())
        bad = {t: r["outcome"] for t, r in rows.items() if r["outcome"] != "pass"}
        print(f"{model:8} {lang:7} {n}/{len(TASKS)} pass  {bad}")
    with open(os.path.join(HERE, "results.json"), "w") as f:
        json.dump(report, f, indent=2)


if __name__ == "__main__":
    main()
