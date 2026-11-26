// Token counts for the same programs in Om, Rust and Python, with real
// byte-pair tokenizers: o200k_base (GPT-4o family) and cl100k_base (GPT-4).
// Claude's tokenizer is not public, so it is not included.
//
// Usage: npm install && node count.mjs        (writes results.json)
//
// Three sets are counted:
//   speed    the seven hand-written programs in ../speed
//   budget   the five hand-written programs in ../../bench/tokens
//   llm      programs a model wrote in ../bugrate/attempts, only tasks where
//            that model's Om, Rust and Python versions all passed
import { getEncoding } from "js-tiktoken";
import fs from "node:fs";
import path from "node:path";
import url from "node:url";

const here = path.dirname(url.fileURLToPath(import.meta.url));
const encs = { o200k: getEncoding("o200k_base"), cl100k: getEncoding("cl100k_base") };
const langs = { om: "om", rust: "rs", python: "py" };
const count = (file) => {
  const src = fs.readFileSync(file, "utf8");
  return Object.fromEntries(Object.entries(encs).map(([k, e]) => [k, e.encode(src).length]));
};

function set(dir, names, ext = (l) => langs[l]) {
  const rows = [];
  for (const n of names) {
    const row = { program: n };
    for (const l of Object.keys(langs)) row[l] = count(path.join(dir, `${n}.${ext(l)}`));
    rows.push(row);
  }
  return rows;
}

const stems = (dir, e) =>
  fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => f.endsWith(e)).map((f) => f.slice(0, -e.length)).sort() : [];

const out = { tokenizers: ["o200k_base", "cl100k_base"], sets: {} };
out.sets.speed = set(path.join(here, "../speed"), stems(path.join(here, "../speed"), ".om"));
out.sets.budget = set(path.join(here, "../../bench/tokens"), stems(path.join(here, "../../bench/tokens"), ".om"));

const bug = path.join(here, "../bugrate");
const res = fs.existsSync(path.join(bug, "results.json")) ? JSON.parse(fs.readFileSync(path.join(bug, "results.json"))) : {};
for (const model of Object.keys(res)) {
  const r = res[model];
  if (!r.om || !r.rust || !r.python) continue;
  const ok = Object.keys(r.om).filter((t) => ["om", "rust", "python"].every((l) => r[l][t].outcome === "pass"));
  out.sets[`llm_${model}`] = ok.map((t) => {
    const row = { program: t };
    for (const [l, e] of Object.entries(langs)) row[l] = count(path.join(bug, "attempts", model, l, `${t}.${e}`));
    return row;
  });
}
out.om_guide = count(path.join(here, "../../docs/guide.md"));

for (const [name, rows] of Object.entries(out.sets)) {
  const tot = { om: 0, rust: 0, python: 0 };
  for (const r of rows) for (const l of Object.keys(tot)) tot[l] += r[l].o200k;
  out.sets[name] = { rows, total_o200k: tot, total_cl100k: Object.fromEntries(Object.keys(tot).map((l) => [l, rows.reduce((s, r) => s + r[l].cl100k, 0)])) };
  console.log(`${name.padEnd(12)} n=${rows.length}  o200k  om ${tot.om}  rust ${tot.rust}  python ${tot.python}  (om/rust ${(tot.om / tot.rust).toFixed(2)}, om/python ${(tot.om / tot.python).toFixed(2)})`);
}
console.log(`om guide (one-time context): ${out.om_guide.o200k} o200k tokens`);
fs.writeFileSync(path.join(here, "results.json"), JSON.stringify(out, null, 2) + "\n");
