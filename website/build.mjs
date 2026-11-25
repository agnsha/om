// Builds the "how to use Om" website from the Markdown in content/ into dist/.
// Run with `npm run build`. The output is plain static files for Cloudflare Pages.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Marked } from "marked";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const SRC = path.join(ROOT, "content");
const OUT = path.join(ROOT, "dist");
const REPO = "https://github.com/agnsha/om";
const P = path.posix;

// ---------- pages and navigation ----------

const walk = (dir) =>
  fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const full = path.join(dir, e.name);
    return e.isDirectory() ? walk(full) : [P.relative(SRC, full).split(path.sep).join("/")];
  });
const files = walk(SRC);
const mdFiles = files.filter((f) => f.endsWith(".md"));

// content path (e.g. "book/README.md") -> output path (e.g. "book/index.html")
function outPath(src) {
  if (src === "README.md") return "learn.html";
  if (src.endsWith("/README.md")) return src.replace(/README\.md$/, "index.html");
  return src.replace(/\.md$/, ".html");
}

const titleOf = (src) => {
  const m = fs.readFileSync(path.join(SRC, src), "utf8").match(/^# (.+)$/m);
  return m ? m[1].replace(/`/g, "") : src;
};

const range = (dir) => mdFiles.filter((f) => f.startsWith(dir + "/") && !f.endsWith("README.md")).sort();

const NAV = [
  ["Start", ["README.md", "install.md", "windows.md", "getting-started.md", "om-for-rust.md"]],
  ["Learn", ["om-card.md", "book/README.md", ...range("book"), "by-example/README.md", ...range("by-example"), "exercises/README.md"]],
  ["Use", ["tools.md", "games.md", "errors.md", "playground.md"]],
  ["Reference", ["reference.md", "docs/language.md", "docs/rust-parity.md", "docs/design.md", "docs/architecture.md"]],
];
const SHORT = {
  "README.md": "Overview",
  "om-card.md": "Language tour (one page)",
  "book/README.md": "The Om Book",
  "by-example/README.md": "Om by Example",
  "exercises/README.md": "Exercises",
  "docs/language.md": "Language reference",
  "docs/rust-parity.md": "Rust parity checklist",
  "docs/design.md": "Design",
  "docs/architecture.md": "Compiler architecture",
};
const BADGE = { "windows.md": "review", "games.md": "review", "playground.md": "planned" };
const ORDER = NAV.flatMap(([, list]) => list);
for (const f of mdFiles) if (!ORDER.includes(f)) console.warn("not in nav:", f);

// ---------- links ----------

const DOC_URL = new RegExp(`^${REPO}/blob/main/docs/([\\w-]+)\\.md`);

function rewriteHref(href, fromSrc) {
  if (/^(mailto:|#)/.test(href)) return href;
  const m = href.match(DOC_URL);
  if (m && fs.existsSync(path.join(SRC, "docs", m[1] + ".md"))) {
    href = P.relative(P.dirname(fromSrc), "docs/" + m[1] + ".md") + href.slice(m[0].length);
  }
  if (/^[a-z]+:/i.test(href)) return href;
  const [p, hash = ""] = href.split("#");
  const target = P.normalize(P.join(P.dirname(fromSrc), p));
  const fromOut = P.dirname(outPath(fromSrc));
  let dest;
  if (target.endsWith(".md") && mdFiles.includes(target)) dest = outPath(target);
  else if (files.includes(target) || fs.existsSync(path.join(ROOT, "static", target))) dest = target; // .om files and images are copied as-is
  else if (fs.existsSync(path.join(SRC, target)) && fs.statSync(path.join(SRC, target)).isDirectory()) dest = target + "/index.html";
  else {
    // Something in the repository but not on the site (examples/, CONTRIBUTING.md ...).
    // content/docs mirrors the repository's docs/, so the path is already repo-relative.
    return `${REPO}/tree/main/${target}${hash ? "#" + hash : ""}`;
  }
  const rel = P.relative(fromOut, dest) || P.basename(dest);
  return rel + (hash ? "#" + hash : "");
}

// ---------- syntax highlighting for Om ----------

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const KW = new Set(
  "fn let mut if else match for in while loop break continue return struct enum impl trait where const as self Self true false defer pub use mod uses type".split(" "),
);
function highlightOm(code) {
  const re = /(\/\/[^\n]*)|("(?:\\.|[^"\\])*")|('(?:\\.|[^'\\])')|(\b\d[\d_]*(?:\.\d+)?(?:[iuf](?:8|16|32|64|size))?\b|\b0x[\da-fA-F_]+\b)|([A-Za-z_]\w*)(\s*\()?/g;
  let out = "", last = 0, m;
  while ((m = re.exec(code))) {
    out += esc(code.slice(last, m.index));
    last = re.lastIndex;
    if (m[1]) out += `<span class="c">${esc(m[1])}</span>`;
    else if (m[2] || m[3]) out += `<span class="s">${esc(m[2] || m[3])}</span>`;
    else if (m[4]) out += `<span class="n">${esc(m[4])}</span>`;
    else {
      const w = m[5];
      let cls = KW.has(w) ? "k" : /^[A-Z]/.test(w) ? "t" : m[6] ? "f" : "";
      out += cls ? `<span class="${cls}">${w}</span>` : w;
      if (m[6]) out += esc(m[6]);
    }
  }
  return out + esc(code.slice(last));
}
function highlightShell(code) {
  return code
    .split("\n")
    .map((line) => {
      const e = esc(line);
      if (/^\s*#/.test(line)) return `<span class="c">${e}</span>`;
      return e.replace(/^(\$ )/, '<span class="pr">$1</span>').replace(/(\s#\s.*)$/, '<span class="c">$1</span>');
    })
    .join("\n");
}

// ---------- markdown ----------

const slug = (s) =>
  s.toLowerCase().replace(/<[^>]+>/g, "").replace(/&[a-z]+;|&#\d+;/g, "").replace(/[^\w\- ]/g, "").replace(/ /g, "-");

function render(src) {
  let md = fs.readFileSync(path.join(SRC, src), "utf8");
  const toc = [];
  const marked = new Marked({ gfm: true });
  marked.use({
    renderer: {
      heading(text, level, raw) {
        const id = slug(raw);
        if (level === 2) toc.push([id, text]);
        if (level === 1) return `<h1>${text}</h1>\n`;
        return `<h${level} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true">#</a>${text}</h${level}>\n`;
      },
      code(code, lang) {
        lang = (lang || "").trim();
        const body = lang === "om" ? highlightOm(code) : /^(sh|console|powershell|bash)$/.test(lang) ? highlightShell(code) : esc(code);
        const label = { om: "Om", sh: "Shell", console: "Terminal", powershell: "PowerShell", text: "Output" }[lang] || lang;
        return `<div class="code">${label ? `<div class="bar"><b>${label}</b><button class="copy" type="button">Copy</button></div>` : ""}<pre><code>${body}</code></pre></div>\n`;
      },
      link(href, title, text) {
        const h = rewriteHref(href, src);
        const ext = /^https?:/.test(h) ? ' rel="noopener"' : "";
        return `<a href="${esc(h)}"${title ? ` title="${esc(title)}"` : ""}${ext}>${text}</a>`;
      },
      image(href, title, text) {
        return `<img src="${esc(rewriteHref(href, src))}" alt="${esc(text)}" loading="lazy">`;
      },
      blockquote(quote) {
        const kind = /<strong>In review/.test(quote) ? "review" : /<strong>(Status: )?planned/i.test(quote) ? "planned" : "";
        return `<blockquote class="${kind}">${quote}</blockquote>\n`;
      },
      table(header, body) {
        return `<div class="table"><table><thead>${header}</thead><tbody>${body}</tbody></table></div>\n`;
      },
    },
  });
  // Bold "planned" / "In review" markers become badges.
  let html = marked.parse(md);
  html = html.replace(/<strong>(planned|Planned)<\/strong>/g, '<span class="badge planned">planned</span>');
  return { html, toc };
}

// ---------- page shell ----------

function rel(fromOut, to) {
  return P.relative(P.dirname(fromOut), to) || P.basename(to);
}

function head(title, desc, here) {
  const css = rel(here, "style.css");
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<meta name="description" content="${esc(desc)}">
<link rel="icon" href="${rel(here, "img/om-mark.svg")}" type="image/svg+xml">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@100..125,400..800&family=JetBrains+Mono:wght@400;600&display=swap" rel="stylesheet">
<link rel="stylesheet" href="${css}">
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="top">
  <a class="logo" href="${rel(here, "index.html")}"><i></i>Om</a>
  <nav aria-label="Main">
    <a href="${rel(here, "install.html")}">Install</a>
    <a href="${rel(here, "getting-started.html")}">Get started</a>
    <a href="${rel(here, "book/index.html")}">Book</a>
    <a href="${rel(here, "by-example/index.html")}">Examples</a>
    <a href="${rel(here, "tools.html")}">Tools</a>
    <a href="${rel(here, "games.html")}">Games</a>
    <a href="${rel(here, "docs/language.html")}">Reference</a>
  </nav>
  <button class="menu" type="button" aria-expanded="false" aria-controls="side">Menu</button>
</header>`;
}

const FOOT = `<footer class="foot"><div>Om is early and experimental. These pages describe the compiler on the main branch; anything marked <span class="badge review">in review</span> is written but not merged yet, and <span class="badge planned">planned</span> is not built.</div><div>Dual-licensed MIT / Apache-2.0.</div></footer>
<script>
document.querySelectorAll(".code .copy").forEach(b=>b.addEventListener("click",()=>{const t=b.closest(".code").querySelector("code").innerText;navigator.clipboard&&navigator.clipboard.writeText(t).then(()=>{b.textContent="Copied";setTimeout(()=>b.textContent="Copy",1400)})}));
const m=document.querySelector(".menu"),s=document.getElementById("side");if(m&&s)m.addEventListener("click",()=>{const o=s.classList.toggle("open");m.setAttribute("aria-expanded",o)});
document.querySelectorAll("[data-tabs]").forEach(g=>{const bs=g.querySelectorAll("[role=tab]"),ps=g.querySelectorAll("[role=tabpanel]");bs.forEach((b,i)=>b.addEventListener("click",()=>{bs.forEach((x,j)=>{x.setAttribute("aria-selected",i===j);ps[j].hidden=i!==j})}))});
</script>
</body>
</html>
`;

function sidebar(here) {
  return `<nav id="side" class="side" aria-label="Pages">${NAV.map(
    ([group, list]) =>
      `<div class="grp"><p>${group}</p><ul>${list
        .map((f) => {
          const o = outPath(f);
          const sub = /^(book|by-example)\/\d/.test(f);
          const label = SHORT[f] || titleOf(f).replace(/^Chapter \d+: /, "");
          const badge = BADGE[f] ? ` <span class="badge ${BADGE[f]}">${BADGE[f] === "review" ? "in review" : "planned"}</span>` : "";
          return `<li${sub ? ' class="sub"' : ""}><a href="${rel(here, o)}"${o === here ? ' aria-current="page"' : ""}>${esc(label)}</a>${badge}</li>`;
        })
        .join("")}</ul></div>`,
  ).join("")}</nav>`;
}

function page(src) {
  const here = outPath(src);
  const { html, toc } = render(src);
  const title = titleOf(src);
  const i = ORDER.indexOf(src);
  const prev = i > 0 ? ORDER[i - 1] : null;
  const next = i >= 0 && i < ORDER.length - 1 ? ORDER[i + 1] : null;
  const pn = `<nav class="pn" aria-label="Previous and next">${prev ? `<a href="${rel(here, outPath(prev))}"><small>Previous</small>${esc(SHORT[prev] || titleOf(prev))}</a>` : "<span></span>"}${next ? `<a class="nx" href="${rel(here, outPath(next))}"><small>Next</small>${esc(SHORT[next] || titleOf(next))}</a>` : ""}</nav>`;
  const onThis = toc.length > 2 ? `<aside class="toc" aria-label="On this page"><p>On this page</p><ul>${toc.map(([id, t]) => `<li><a href="#${id}">${t}</a></li>`).join("")}</ul></aside>` : "";
  return `${head(title + " · Om", "How to use the Om programming language: " + title, here)}
<div class="layout">
${sidebar(here)}
<main id="main" class="doc">
<article class="prose">${html}</article>
${pn}
</main>
${onThis}
</div>
${FOOT}`;
}

// ---------- home page ----------

function home() {
  const here = "index.html";
  const tab = (id, label, sel) => `<button role="tab" id="t-${id}" aria-controls="p-${id}" aria-selected="${sel}" type="button">${label}</button>`;
  const panel = (id, lang, code, note, hidden) =>
    `<div role="tabpanel" id="p-${id}" aria-labelledby="t-${id}"${hidden ? " hidden" : ""}><div class="code"><div class="bar"><b>${lang}</b><button class="copy" type="button">Copy</button></div><pre><code>${highlightShell(code)}</code></pre></div><p class="note">${note}</p></div>`;
  const card = (href, kicker, title, text) => `<a class="card" href="${href}"><small>${kicker}</small><b>${title}</b><span>${text}</span></a>`;
  const hello = `fn fib(n: i64) -> i64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

fn main() {
    let name = "Om"
    print("Hello, {name}!")
    print("fib(20) = {fib(20)}")
}`;
  return `${head("Om: how to use the Om programming language", "Install Om, write your first program, learn the language and its tools. Om is a compiled systems language with Rust's memory model and less to write.", here)}
<main id="main">
<section class="hero band">
  <div class="wrap hero-grid">
    <div>
      <p class="eyebrow">The Om programming language</p>
      <h1>Learn to write Om<span>A compiled systems language with Rust's ownership model, no garbage collector, no lifetime annotations, and less to type.</span></h1>
      <div class="cta">
        <a class="btn p" href="install.html">Install Om</a>
        <a class="btn" href="getting-started.html">Your first program</a>
      </div>
      <p class="status">Early and experimental. Everything on this site runs on today's compiler unless it is marked <span class="badge review">in review</span> or <span class="badge planned">planned</span>.</p>
    </div>
    <div class="term">
      <div class="bar"><b>hello.om</b></div>
      <pre><code>${highlightOm(hello)}</code></pre>
      <pre class="out"><code><span class="pr">$</span> om run hello.om
Hello, Om!
fib(20) = 6765</code></pre>
    </div>
  </div>
</section>

<section class="band alt" id="install">
  <div class="wrap">
    <h2>Install in a few minutes</h2>
    <p>Om is built from source for now. You need <a href="https://rustup.rs" rel="noopener">Rust</a>, Git and a C compiler for linking. The repository is private, so cloning needs a GitHub account with access.</p>
    <div class="tabs" data-tabs>
      <div role="tablist" aria-label="Operating system">${tab("linux", "Linux", true)}${tab("mac", "macOS", false)}${tab("win", "Windows", false)}</div>
      ${panel("linux", "Shell", `# Debian / Ubuntu (Fedora: sudo dnf install gcc git)
sudo apt install build-essential git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone ${REPO}
cd om
cargo install --path crates/omc
om run examples/hello.om`, `Full steps and troubleshooting: <a href="install.html#linux">Install on Linux</a>.`, false)}
      ${panel("mac", "Shell", `xcode-select --install          # clang and git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone ${REPO}
cd om
cargo install --path crates/omc
om run examples/hello.om`, `Full steps: <a href="install.html#macos">Install on macOS</a>.`, true)}
      ${panel("win", "PowerShell", `# Today: run Om inside WSL
wsl --install                   # as administrator, then restart
# open the Ubuntu app and follow the Linux steps`, `On main today, Windows uses WSL. Native PowerShell support (no WSL) is <span class="badge review">in review</span>: see <a href="windows.html">Om on Windows</a>.`, true)}
    </div>
  </div>
</section>

<section class="band" id="learn">
  <div class="wrap">
    <h2>Pick a way to learn</h2>
    <div class="cards">
      ${card("getting-started.html", "10 minutes", "Getting started", "Write, run and build your first program, and read your first compiler error.")}
      ${card("om-card.html", "Language tour", "Om in one page", "The whole language on one page. Small enough to paste into an AI assistant.")}
      ${card("book/index.html", "11 chapters", "The Om Book", "From hello to ownership, traits and generics. Start here if you're new.")}
      ${card("by-example/index.html", "18 programs", "Om by Example", "Short, complete programs, one topic each, with their real output.")}
      ${card("exercises/index.html", "12 exercises", "Exercises", "Small broken programs to fix. Learn by getting the compiler to say yes.")}
      ${card("om-for-rust.html", "One page", "Om for Rust programmers", "Already know Rust? Every difference that matters.")}
    </div>
  </div>
</section>

<section class="band alt" id="tools">
  <div class="wrap two">
    <div>
      <h2>One command does it all</h2>
      <p>The <code>om</code> command compiles your code to a native executable through Cranelift. There is no interpreter and no virtual machine.</p>
      <p><a class="more" href="tools.html">All commands and flags</a></p>
    </div>
    <div class="code"><div class="bar"><b>Shell</b></div><pre><code>${highlightShell(`om run   app.om          # compile and run
om build app.om -o app   # standalone executable
om check app.om          # type-check only, fastest`)}</code></pre></div>
  </div>
</section>

<section class="band" id="games">
  <div class="wrap">
    <h2>Two games written in Om <span class="badge review">in review</span></h2>
    <p>Emberdeep, a first-person 3D descent drawn in the terminal, and The Hollow Bell, a stealth roguelike of sound and spoken spells. Each is one Om file of a couple of thousand lines.</p>
    <a class="shot" href="games.html"><img src="img/emberdeep-ember.png" alt="Emberdeep: ember light on a brick corridor" loading="lazy"><img src="img/emberdeep-gate.png" alt="Emberdeep: the open gate" loading="lazy"><img src="img/emberdeep-hound.png" alt="Emberdeep: a hound in the caves" loading="lazy"></a>
    <p><a class="more" href="games.html">How to run and play them</a></p>
  </div>
</section>
</main>
${FOOT}`;
}

// ---------- write everything ----------

fs.rmSync(OUT, { recursive: true, force: true });
const write = (p, s) => {
  fs.mkdirSync(path.dirname(path.join(OUT, p)), { recursive: true });
  fs.writeFileSync(path.join(OUT, p), s);
};
for (const f of mdFiles) write(outPath(f), page(f));
for (const f of files.filter((f) => !f.endsWith(".md"))) write(f, fs.readFileSync(path.join(SRC, f)));
// Directory listing for exercise solutions.
const sol = files.filter((f) => f.startsWith("exercises/solutions/"));
write(
  "exercises/solutions/index.html",
  `${head("Exercise solutions · Om", "Solutions to the Om exercises", "exercises/solutions/index.html")}<div class="layout">${sidebar("exercises/solutions/index.html")}<main id="main" class="doc"><article class="prose"><h1>Exercise solutions</h1><p>Try each exercise first. These are the fixed programs.</p><ul>${sol.map((f) => `<li><a href="${P.basename(f)}"><code>${P.basename(f)}</code></a></li>`).join("")}</ul><p><a href="../index.html">Back to the exercises</a></p></article></main></div>${FOOT}`,
);
write("index.html", home());
for (const f of fs.readdirSync(path.join(ROOT, "static"), { recursive: true })) {
  const full = path.join(ROOT, "static", f);
  if (fs.statSync(full).isFile()) write(f, fs.readFileSync(full));
}
write(
  "404.html",
  `${head("Not found · Om", "Page not found", "404.html")}<main id="main" class="band"><div class="wrap"><h1>Page not found</h1><p>That page doesn't exist. Try the <a href="/">home page</a> or <a href="/learn.html">the overview</a>.</p></div></main>${FOOT}`,
);
console.log(`built ${mdFiles.length + 3} pages into dist/`);
