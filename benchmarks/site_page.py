"""Build the website's benchmarks.html from the results, reusing the header,
sidebar and footer of an existing page of the built site.

Usage: python3 site_page.py path/to/dist/tools.html > path/to/dist/benchmarks.html
"""

import html
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
load = lambda p: json.load(open(os.path.join(HERE, p)))
speed, tokens, bugs = load("speed/results.json"), load("tokens/results.json"), load("bugrate/results.json")
tpl = open(sys.argv[1], encoding="utf-8").read()
NAMES = {"sonnet": "Claude Sonnet", "haiku": "Claude Haiku"}
REPO = "https://github.com/agnsha/om/blob/main/benchmarks"
e = html.escape

res = speed["results"]
geo = lambda k: math.prod(r["run_s"][k] for r in res) ** (1 / len(res))
om_rs, py_om = geo("om") / geo("rs"), geo("py") / geo("om")
llm = {k[4:]: v for k, v in tokens["sets"].items() if k.startswith("llm_")}
tok_ratio = sum(v["total_o200k"]["om"] for v in llm.values()) / sum(v["total_o200k"]["rust"] for v in llm.values())
passes = {m: {l: sum(x["outcome"] == "pass" for x in r[l].values()) for l in r} for m, r in bugs.items()}
om_pass = sum(p["om"] for p in passes.values())
total = sum(len(r["om"]) for r in bugs.values())

def bar_row(label, vals, unit, scale):
    cells = "".join(
        f'<div class="bx-l {k}"><span class="bx-k">{n}</span><span class="bx-t"><i style="width:{max(1.5, 100 * v / scale):.1f}%"></i></span><span class="bx-v">{v:{unit}}</span></div>'
        for k, n, v in vals)
    return f'<div class="bx-row"><div class="bx-h">{e(label)}</div>{cells}</div>'

speed_rows = []
for r in res:
    t = r["run_s"]
    m = max(t["om"], t["rs"])
    speed_rows.append(bar_row(f'{r["program"]} ({r["arg"]})', [("om", "Om", t["om"]), ("rs", "Rust", t["rs"])], ".3f", m * 1.05))

speed_tbl = "".join(
    f'<tr><td><code>{r["program"]}</code></td><td>{r["run_s"]["om"]:.3f}</td><td>{r["run_s"]["rs"]:.3f}</td>'
    f'<td>{r["run_s"]["rs_checked"]:.3f}</td><td>{r["run_s"]["py"]:.2f}</td><td>{r["run_s"]["om"] / r["run_s"]["rs"]:.1f}x</td></tr>'
    for r in res)

set_names = {"speed": "Speed programs, hand-written", "budget": "Token-budget programs, hand-written"}
tok_tbl = ""
for k, s in tokens["sets"].items():
    t = s["total_o200k"]
    name = set_names.get(k, f"Written by {NAMES.get(k[4:], k[4:])}")
    tok_tbl += (f'<tr><td>{e(name)} <small>({len(s["rows"])} programs)</small></td><td>{t["om"]}</td><td>{t["rust"]}</td>'
                f'<td>{t["python"]}</td><td>{t["om"] / t["rust"]:.2f}</td></tr>')

bug_tbl = "".join(
    f'<tr><td>{NAMES.get(m, m)}</td><td>{p["om"]}/12</td><td>{p["rust"]}/12</td><td>{p["python"]}/12</td></tr>'
    for m, p in passes.items())
code = lambda t: re.sub(r"`([^`]*)`", r"<code>\1</code>", e(t))
fails = "".join(
    f'<li><code>{t}</code> ({NAMES.get(m, m)}): {code(x["detail"].splitlines()[0].replace("error", "", 1).lstrip("[]E0123456789: ") if x.get("detail") else x["outcome"])}</li>'
    for m, r in bugs.items() for l, rows in r.items() for t, x in rows.items() if x["outcome"] != "pass")

m = speed["meta"]
body = f"""<article class="prose"><h1>Benchmarks</h1>
<p class="lede">Om against Rust and Python on three things: how fast the code runs, how many LLM tokens it costs, and how often a model gets it right on the first try. Measured, not projected, and Om loses some of it.</p>
<div class="bx-tiles">
<div class="bx-tile"><b>{om_rs:.1f}x</b><span>slower than Rust<br>(geometric mean, 7 programs)</span></div>
<div class="bx-tile sig"><b>{py_om:.0f}x</b><span>faster than Python<br>(same programs)</span></div>
<div class="bx-tile"><b>{tok_ratio:.2f}x</b><span>Rust's tokens, in code<br>written by an LLM</span></div>
<div class="bx-tile"><b>{om_pass}/{total}</b><span>Om tasks right first try<br>(Rust and Python: {total}/{total})</span></div>
</div>
<div class="note"><p><b>Where this stands.</b> Om only has the Cranelift backend today, built for fast compiles rather than fast code. The LLVM release backend is on the roadmap, and these numbers will be re-run when it lands.</p></div>

<h2 id="speed"><a class="anchor" href="#speed" aria-hidden="true">#</a>Speed</h2>
<p>Seven programs, written the same way in all three languages with plain loops and arrays. Wall time in seconds, median of {m["runs"]} runs, process start included. Shorter bars are faster.</p>
<div class="bx">{"".join(speed_rows)}</div>
<div class="table"><table><thead><tr><th>Program</th><th>Om</th><th>Rust <code>-O</code></th><th>Rust + overflow checks</th><th>Python</th><th>Om vs Rust</th></tr></thead><tbody>{speed_tbl}</tbody></table></div>
<p>Om checks integer overflow in every build and release Rust does not, so Rust is also shown with <code>-C overflow-checks=on</code>. That explains part of the gap, not most of it. Om is closest on simple loops (<code>sieve</code>) and furthest where LLVM vectorizes (<code>matmul</code>) or where Rust's library is tuned (<code>sort</code>, string building). Builds take about the same time in both, 0.2 to 0.3 seconds, mostly linking.</p>

<h2 id="tokens"><a class="anchor" href="#tokens" aria-hidden="true">#</a>Tokens</h2>
<p>Counted with real tokenizers (<code>o200k_base</code> for GPT-4o, and <code>cl100k_base</code>) rather than an estimate. Claude's tokenizer is not public. Totals below are o200k; lower is better.</p>
<div class="table"><table><thead><tr><th>Programs</th><th>Om</th><th>Rust</th><th>Python</th><th>Om / Rust</th></tr></thead><tbody>{tok_tbl}</tbody></table></div>
<p>Hand-written Om is about 6 to 9% shorter than Rust. In code the models wrote, Om came out slightly <em>longer</em>: without iterators, closures or a map type yet, they write manual loops. Python is the most compact everywhere. A model also needs the Om guide in context once per session, which costs <b>{tokens["om_guide"]["o200k"]} tokens</b>.</p>

<h2 id="bug-rate"><a class="anchor" href="#bug-rate" aria-hidden="true">#</a>Bug rate</h2>
<p>Each model wrote the same 12 tasks in each language from the same text, in one shot, with no compiler feedback. The Om prompt included the Om guide. A task passes if it builds and prints exactly the expected output.</p>
<div class="table"><table><thead><tr><th>Model</th><th>Om</th><th>Rust</th><th>Python</th></tr></thead><tbody>{bug_tbl}</tbody></table></div>
<p>Every Om failure was a compile error, never a wrong answer, and each points at a gap in Om rather than at the task:</p>
<ul>{fails}</ul>
<p>The fixes are on the list: <code>i64</code> should satisfy a <code>Clone</code> bound, plain strings should not trip on <code>{{}}</code>, integer literals should infer <code>usize</code> from later use, and <code>Box[T]</code> should deref to <code>&amp;T</code>.</p>

<h2 id="reproduce"><a class="anchor" href="#reproduce" aria-hidden="true">#</a>Run it yourself</h2>
<p>Everything is in the repository: programs, prompts, the models' raw replies and the extracted programs. See <a href="{REPO}/README.md" rel="noopener">the method</a> and <a href="{REPO}/RESULTS.md" rel="noopener">the full results</a>.</p>
<div class="code"><div class="bar"><b>Shell</b><button class="copy" type="button">Copy</button></div><pre><code>cargo build --release -p omc
python3 benchmarks/speed/run.py
python3 benchmarks/bugrate/run.py
(cd benchmarks/tokens &amp;&amp; npm install &amp;&amp; node count.mjs)
python3 benchmarks/report.py</code></pre></div>
<p><small>Measured on a 4-core Linux x86-64 cloud VM with {e(m["om"])}, {e(m["rustc"])} and Python {e(m["python"])}.</small></p>
</article>"""

style = """<style>
.bx-tiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;margin:22px 0}
.bx-tile{background:var(--panel);border:1px solid var(--line);padding:14px 16px}
.bx-tile b{display:block;font:600 30px/1.1 var(--display);letter-spacing:-.03em;color:var(--ink)}
.bx-tile.sig b{color:var(--sig)}
.bx-tile span{display:block;margin-top:6px;color:var(--dim);font-size:13.5px;line-height:1.4}
.bx{display:grid;gap:14px;margin:18px 0 22px}
.bx-h{font:500 13px var(--mono);color:var(--dim);margin-bottom:4px}
.bx-l{display:grid;grid-template-columns:44px 1fr 56px;align-items:center;gap:10px;font-size:13px}
.bx-k{color:var(--dim)}
.bx-t{height:10px;background:var(--hull);border:1px solid var(--line)}
.bx-t i{display:block;height:100%;background:var(--ion)}
.bx-l.om .bx-t i{background:var(--sig)}
.bx-v{font-family:var(--mono);text-align:right;color:var(--ink)}
.prose .lede{font-size:1.1em;color:var(--dim)}
</style>"""

page = tpl
page = re.sub(r"<title>.*?</title>", "<title>Benchmarks · Om</title>", page, flags=re.S)
page = re.sub(r'<meta name="description" content="[^"]*">',
              '<meta name="description" content="Om against Rust and Python: runtime speed, LLM token counts and first-try bug rate, measured and reproducible.">', page)
page = page.replace("</head>", style + "\n</head>", 1)
page = page.replace(' aria-current="page"', "")
page = re.sub(r'<article class="prose">.*?</article>', lambda _: body, page, count=1, flags=re.S)
page = re.sub(r'<nav class="pn".*?</nav>', '<nav class="pn" aria-label="Previous and next"><a href="tools.html"><small>Previous</small>Tools</a><a class="nx" href="games.html"><small>Next</small>Games written in Om</a></nav>', page, count=1, flags=re.S)
toc = '<ul><li><a href="#speed">Speed</a></li><li><a href="#tokens">Tokens</a></li><li><a href="#bug-rate">Bug rate</a></li><li><a href="#reproduce">Run it yourself</a></li></ul>'
page = re.sub(r'(<aside class="toc"[^>]*><p>On this page</p>)<ul>.*?</ul>', lambda mm: mm.group(1) + toc, page, count=1, flags=re.S)
page = page.replace('<li><a href="benchmarks.html">Benchmarks</a></li>', '<li><a href="benchmarks.html" aria-current="page">Benchmarks</a></li>')
sys.stdout.write(page)
