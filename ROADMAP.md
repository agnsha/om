# Om roadmap: what it takes to be viable

This is a prioritized list of work that would make Om worth choosing over Rust
or Python. It covers four goals (AI usage, token saving, safety, speed) plus the
adoption blockers that stop anyone from using Om at all. It builds on the phase
table in [README.md](README.md#roadmap) and [docs/design.md](docs/design.md); it
does not replace them.

The order comes from evidence first. The [benchmarks](benchmarks/RESULTS.md)
say three things today:

- **Bug rate:** Claude Haiku passed 9/12 tasks in Om and Sonnet 10/12, against
  12/12 in Rust and Python. All five failures were compile errors from four
  language gaps, not model mistakes.
- **Tokens:** model-written Om is about 6 to 7% *longer* than Rust (1.06x to
  1.07x), because there are no iterators, closures or `HashMap`. Om also costs a
  one-time 2,345-token guide per session.
- **Speed:** Om is 3.5x slower than optimized Rust (geometric mean), from 1.1x
  on `sieve` to 7.8x on `matmul`.

So the first section fixes what the benchmarks measured, and each later
section is ordered by how much it should move those numbers.

**Key.** Effort: **S** is days, **M** is weeks, **L** is a month or more of
focused work. Priority: **P0** blocks the stated goals or a benchmark number,
**P1** is needed before anyone outside the project uses Om, **P2** is worth
doing later. Items marked *(speculative)* rest on research results or bets
that have not been shown to hold for Om; treat them as experiments with a
measurement attached, not commitments.

---

## 0. Close the gaps the benchmarks found

These come first because each one is a measured failure or a measured token
cost, and each is cheap compared with what it buys.

| # | Item | Why | Effort | Pri |
|---|---|---|---|---|
| 0.1 | Built-in scalars (`i64`, `f64`, `bool`, `char`, ...) implement `Clone`, `Copy`, `Eq`, `Ord`, `Default` as traits, not just methods | Both models failed `05_stack` on `impl[T: Clone]` rejecting `Stack[i64]` | S | P0 |
| 0.2 | Plain string literals do not interpolate; interpolation needs a marker (`f"..."`, `$"..."`) or only triggers on `{ident...}` | `"([]{})"` is a compile error today, a trap no mainstream language has (`09_brackets`) | S | P0 |
| 0.3 | Integer literal inference from later use (`let mut lo = 0` becomes `usize` when compared with `len()`) | Caused `07_binary_search`; also hit both games (signed coordinates vs `usize` indexes) | M | P0 |
| 0.4 | Auto-deref `Box[T]` to `&T` at call sites (deref coercion), and `&Vec[T]` to `&[T]` once slices exist | Caused `11_expr_tree`; Rust users and models expect it | S | P0 |
| 0.5 | Closures (`\|x\| x + 1`) with capture by reference and by move | Prerequisite for iterators, `sort_by_key`, `retain`; named in both game notes | L | P0 |
| 0.6 | Iterators and adapters: `iter`, `map`, `filter`, `enumerate`, `zip`, `sum`, `collect`, `any`, `all`, `fold` | The main reason model-written Om is longer than Rust | L | P0 |
| 0.7 | `HashMap` and `HashSet` in the prelude | Models write manual linear search without them; both games worked around it | M | P0 |
| 0.8 | Make `om fix --maybe` actually repair the four benchmark errors, and test that it does | It repaired 0 of 5; the "fix" for 0.2 broke other literals on the line | S | P0 |
| 0.9 | Re-run the bug-rate benchmark *with* compiler feedback (one or two repair rounds) as a second column | Real agents compile and retry; this measures what `--json` errors and `om fix` are worth | S | P0 |
| 0.10 | Grow the bug-rate suite from 12 to 50+ tasks, with more models, and publish pass@1 with confidence intervals | 12 tasks means one failure moves the score 8 points; the current gap might be noise or might be larger | M | P0 |
| 0.11 | Gate CI on the benchmarks: token budget already gated; add bug-rate (cached responses) and speed regression checks | Stops later work from silently undoing these wins | S | P1 |
| 0.12 | Fix the remaining game-note pain points: `for` over `&self.list` borrowing all of `self`, fixed-size arrays `[T; N]`, `write!`-style formatting into an existing `String`, `print` without a newline, `eprint`/`dbg`, `const` in patterns, tuples implementing `Ord` | Each cost a workaround in a 2,000-line real program; models will hit the same walls | M | P0 |

## 1. AI usage: models write correct Om on the first try

The goal is for a model that has never seen Om to match its Rust pass rate.
Om's advantage has to come from tooling, because it will never have Rust's
training data. Research on low-resource languages finds that in-context
documentation and compiler feedback help, but no single trick closes the gap
([Giagnorio et al. 2025](https://arxiv.org/html/2501.19085v1)).

| # | Item | Why | Effort | Pri |
|---|---|---|---|---|
| 1.1 | "Rust-shaped by default": wherever Om differs from Rust for no strong reason, accept the Rust spelling too (`Vec<T>` as well as `Vec[T]`, `Vec::new()`, `&str`, `.iter()`), or give an `auto` fix that rewrites it | Models' priors are Rust; every gratuitous difference is a compile round trip. Cheapest way to borrow Rust's training data | M | P0 |
| 1.2 | Measure which Rust spellings models emit in Om (mine the failed and passed attempts in `benchmarks/bugrate/attempts`) and turn each into either accepted syntax or an `auto` fix | Turns 1.1 from guesswork into a ranked list | S | P0 |
| 1.3 | Language server (LSP): diagnostics, hover types, go to definition, completion, rename | Every coding agent and editor (Claude Code, Cursor, VS Code) uses LSP for context; without it Om is invisible to them | L | P0 |
| 1.4 | MCP server for the compiler: tools `check`, `fix`, `doc`, `test`, `explain`, returning the existing JSON | Lets agents call the compiler directly with structured output instead of parsing a terminal | S | P1 |
| 1.5 | Publish an EBNF/GBNF/Lark grammar of Om and keep it tested against the parser | Enables grammar-constrained decoding (XGrammar, llguidance, llama.cpp GBNF) so a model cannot emit unparseable Om ([XGrammar](https://arxiv.org/pdf/2411.15100)) | M | P1 |
| 1.6 | *(speculative)* Type-constrained decoding: expose an incremental "which tokens are type-valid here" API from the checker | Research on TypeScript cut compile errors by more than half this way ([Mündler et al., PLDI 2025](https://arxiv.org/abs/2504.09246)). Only works with self-hosted models, and needs an incremental checker | L | P2 |
| 1.7 | `om guide --for <task>`: a task-scoped slice of the guide (strings, collections, errors, ownership) instead of all 2,345 tokens | Cuts the fixed per-session cost and focuses the model; MoonBit argues for small, local context in AI-oriented design ([MoonBit](https://www.moonbitlang.com/blog/ai-coding)) | S | P1 |
| 1.8 | `llms.txt` and a one-page "Om for Rust programmers" diff at a stable URL | Agents with web access fetch these; the diff format plays to models' Rust knowledge | S | P1 |
| 1.9 | Every error message includes the exact replacement text, never only prose ("write `x as usize`", with a fix span) | Machine-applicable fixes are what let an agent loop converge in one step | M | P0 |
| 1.10 | Error budget: rank the top 20 error codes from the benchmark attempts and make each one's `auto` fix rate a tracked number | Focuses diagnostic work on what models actually trip on | S | P1 |
| 1.11 | Order-independent, fully local type checking: a function's signature is enough to check a call (no inference across functions) | An agent editing one function should not need to read the whole file; already mostly true, worth guaranteeing | S | P1 |
| 1.12 | `om outline file.om`: signatures, types and doc comments only, no bodies | Agents can read a 2,000-line file's API in a few hundred tokens | S | P1 |
| 1.13 | `om test --json` failures include the expected/actual diff and the failing line, and `om run` panics print a short backtrace | Runtime failures need the same structure that compile errors already have | S | P1 |
| 1.14 | Snapshot/golden tests (`om test --update`) and property tests (`check(fn(x: i64) {...})`) | Agents verify their own code faster with built-in test styles | M | P2 |
| 1.15 | Doc comments `///` and `om doc` for user code | Agents learn a project's API from `om doc`, the same way they learn the standard library | S | P1 |
| 1.16 | *(speculative)* Publish a fine-tuning/eval dataset of Om programs (examples, games, learn/, benchmark references) with a permissive license | Gives future models some Om in their training data; cheap to do, payoff is years out and uncertain | S | P2 |
| 1.17 | Deterministic, stable output everywhere (sorted diagnostics, no timestamps, stable error codes across versions) | Agents cache and compare outputs; churn wastes tokens and breaks scripts | S | P1 |

## 2. Token saving: Om costs fewer tokens than Rust, end to end

Hand-written Om already beats Rust by 6 to 9%, but model-written Om loses by 6
to 7%, and Python beats both. The goal is to win on *model-written* code,
including the guide's fixed cost.

| # | Item | Why | Effort | Pri |
|---|---|---|---|---|
| 2.1 | Iterators, closures, `HashMap` (0.5 to 0.7) | The measured cause of the model-written token gap | L | P0 |
| 2.2 | Measure tokens on Claude's tokenizer via the token-counting API, not only `o200k`/`cl100k` | Om is sold as AI-friendly; the main users' tokenizer is not in the numbers yet | S | P0 |
| 2.3 | Count *total session tokens* (guide + code + error messages + repair rounds), not just final code | A language with longer code but zero retries can be cheaper overall; this is the number users pay | M | P0 |
| 2.4 | Shrink the guide: cut it to the minimum that keeps the pass rate, measured by ablation | The 2,345-token guide is paid every session; each 500 tokens cut is worth more than many syntax tweaks | M | P1 |
| 2.5 | `OM_AGENT=1` as the default when stdout is not a TTY, with errors trimmed to code + location + fix | Agents should not pay for ASCII-art source snippets they cannot see | S | P1 |
| 2.6 | `om test` and `om build` print nothing on success (or one line) | Success output is pure token cost in an agent loop | S | P1 |
| 2.7 | Tokenizer-aware keyword and stdlib names: prefer names that are one token in common tokenizers (`len`, `push`, `map`) and avoid ones that split | Small per-use saving that compounds; check with `om tokens` before adding names | S | P2 |
| 2.8 | Shorthands with measured savings: `let else`, struct update `..base`, tuple structs, `type` aliases, implicit `Ok(())` at the end of `-> Result[(), E]` functions | Each removes a common boilerplate line Rust also has | M | P1 |
| 2.9 | Derive-free traits: let `==`, `<`, `hash`, `clone` work structurally on plain data without `impl` blocks, as printing already does | Removes `#[derive(...)]`-equivalent lines models would otherwise write | M | P1 |
| 2.10 | *(speculative)* Indentation-optional syntax or dropping braces | Python's lead is partly indentation, but braces help models keep structure in long files and make constrained decoding easier. Measure before deciding; the default should be "no" | M | P2 |
| 2.11 | Error conversion through `From` in `?` | Removes `.map_err(...)` noise on every fallible call | M | P1 |
| 2.12 | Re-run the token benchmark per release and publish the trend | Keeps the "saves tokens" claim honest as features land | S | P1 |

## 3. Safety: safe by default, including against the agent

Om already has no build-time code execution, overflow checks in every build,
checked effects (`uses io`, `uses panic`) and a fuzzed compiler. The work is to
finish memory safety and make effects a real capability system, which matters
more when code is written by a model and run unattended.

| # | Item | Why | Effort | Pri |
|---|---|---|---|---|
| 3.1 | Finish the borrow checker: field-level borrows (fixes the `for` over `&self.list` pain), flow-sensitive borrows like Rust's NLL, a written soundness argument for second-class references | Today it is a "conservative stand-in"; unsound holes in a safety-first language are fatal, and over-strictness costs agent round trips | L | P0 |
| 3.2 | Run the existing test suite under AddressSanitizer and Valgrind in CI, plus a Miri-style interpreter mode for the prelude's `Ptr[T]` code | The prelude uses raw pointers; memory bugs there break every program's safety | M | P0 |
| 3.3 | Grow the compiler fuzzer: grammar-based generation (from 1.5), differential testing of Om vs an Om interpreter, and continuous fuzzing (OSS-Fuzz if eligible) | A compiler that miscompiles is a safety bug; differential testing finds miscompiles plain fuzzing cannot | M | P1 |
| 3.4 | Capabilities instead of ambient authority: `main` receives `fs`, `net`, `env`, `clock` values and passes them down; a function without one cannot do I/O | Turns `uses io` into something enforceable per call site, so an agent can see that a parser cannot open sockets. Austral shows this works in a systems language ([Austral](https://borretti.me/article/introducing-austral)) | L | P1 |
| 3.5 | `om audit --deny net,fs` for packages, enforced at build time | With capabilities, a dependency's permissions become a checkable manifest line, a defense against supply-chain attacks | M | P1 |
| 3.6 | Sandboxed `om run --sandbox` (seccomp/landlock on Linux, or a WASI build target) | Agents run untrusted generated code constantly; a built-in sandbox is safer than every user configuring one | M | P1 |
| 3.7 | User-facing `unsafe` blocks, with `om audit` listing every one, and a lint that unsafe code has a `// SAFETY:` comment | Needed for FFI and performance; making it countable keeps it rare | M | P1 |
| 3.8 | Integer overflow policy per type or per block (`wrapping { ... }`), keeping checks on by default | Lets hot loops opt out explicitly instead of the whole build | S | P2 |
| 3.9 | Contracts: `requires`/`ensures`/`invariant` checked at runtime in debug and test builds | Cheap to add, gives agents a precise way to state intent, and is the hook for 3.10 | M | P2 |
| 3.10 | *(speculative)* Static verification of contracts through an SMT solver (Verus-style), with models writing the proof annotations | Research shows LLMs can generate proofs for Verus a good share of the time ([AutoVerus](https://arxiv.org/html/2409.13082v3)); for Om this is a research project, not a feature | L | P2 |
| 3.11 | Data-race freedom for the planned concurrency: structured concurrency with `Send`/`Sync`-style checks before any threads ship | Retrofitting race safety later is close to impossible | L | P1 |
| 3.12 | Reproducible builds and a lockfile with hashes (with 4.x package manager) | Supply-chain safety; agents add dependencies freely | M | P1 |
| 3.13 | A written security model and threat list for "compiling untrusted Om" (limits, no build-time code, resource caps) | The design claims exist in docs/design.md; a threat model makes them testable | S | P2 |

## 4. Speed: match Rust, then beat it where the design allows

Om is 3.5x slower than `rustc -O` today. Most of that is the backend: Cranelift
is built for fast compilation, not peak code quality, and published comparisons
put its output noticeably behind LLVM's
([Wasmtime/Cranelift discussion](https://bytecodealliance.github.io/zulip-archive/stream/217117-cranelift/topic/Comparisons.20between.20cranelift.20and.20LLVM.3F.html),
[Rust project goal on cg_clif](https://rust-lang.github.io/rust-project-goals/2026/improve-cg_clif-performance.html)).
"Faster than Rust" is only plausible in the places design.md names: allocation
patterns (regions) and compile time. Raw compute can at best tie.

| # | Item | Why | Effort | Pri |
|---|---|---|---|---|
| 4.1 | Profile the seven speed benchmarks and attribute the gap (backend, overflow checks, bounds checks, prelude code, allocation, missing inlining) | The plan below should follow measured causes; today only "overflow checks are part of it" is known | S | P0 |
| 4.2 | `om-ir`: a typed, monomorphized SSA IR between checking and codegen | Prerequisite for any optimization Om owns and for a second backend; already planned for phase 3 | L | P0 |
| 4.3 | LLVM release backend (`om build --release`), Cranelift for dev builds | Closes most of the gap on `matmul` (vectorization) and `fib`/`collatz` (better inlining, register allocation). Same split Rust is moving towards | L | P0 |
| 4.4 | Bounds-check and overflow-check elimination in `om-ir` (range analysis, loop induction variables) | Om keeps checks on in every build, so it must remove the provably dead ones or it always loses to Rust's release builds | M | P0 |
| 4.5 | Faster prelude: `sort` (pattern-defeating quicksort / driftsort-style), `String` building with capacity growth and small-string handling | `sort` is 5.6x and `strings` 5.6x slower; these are library, not backend, gaps | M | P0 |
| 4.6 | Inlining across the prelude and generic code, and `#[inline]`-like hints | Small functions in `prelude.om` (`Vec::get`, `len`) must vanish to compete | M | P1 |
| 4.7 | Fixed-size arrays and slices `[T; N]`, `&[T]` on the stack | The Emberdeep notes show every `[a, b, c][i]` allocating a heap `Vec` | M | P0 |
| 4.8 | Regions (arena per scope / per request) as designed | The one place Om can credibly beat Rust's defaults on allocation-heavy code; needs a benchmark that shows it | L | P1 |
| 4.9 | Escape analysis to put non-escaping `Box`/`Vec` on the stack | Cuts allocations in idiomatic code without user changes | M | P2 |
| 4.10 | Layout optimizations: niche filling (`Option[Box[T]]` the size of a pointer), field reordering, enum packing | Rust gets these for free; they matter for cache use in data-heavy code | M | P1 |
| 4.11 | SIMD types and auto-vectorization-friendly codegen (`f64x4`, or rely on LLVM) | `matmul` and `nbody` are the biggest gaps; LLVM alone may close most of it | M | P2 |
| 4.12 | Fast compile: incremental per-function builds and caching, faster linking (`mold`/`lld`), parallel codegen | Builds are 0.23 to 0.31s, no faster than `rustc` on tiny programs, mostly linking; dev-loop speed is a promised win | M | P1 |
| 4.13 | Profile-guided optimization and LTO in release mode | Standard 5 to 15% wins in LLVM-based toolchains; easy once 4.3 exists | S | P2 |
| 4.14 | Benchmark against C and Go too, publish method and raw data per release, losses included | Already the policy in design.md; keeps "faster than Rust" from becoming an unbacked claim | S | P1 |
| 4.15 | *(speculative)* Om-specific optimizations from its stricter semantics: no aliasing of `&mut` (like Rust's `noalias`), no references in structs, value semantics, enabling more aggressive alias analysis than Rust's | This is the only path to *beating* Rust on compute, and it is unproven; worth an experiment after 4.3 | L | P2 |

## 5. Adoption blockers: things that stop anyone using Om at all

None of the four goals matters if nobody can build a real project. These are
not optional; they are just less specific to Om's pitch.

| # | Item | Why | Effort | Pri |
|---|---|---|---|---|
| 5.1 | Modules, `use`, `pub`, multi-file programs | Both games are single 2,000+ line files; nothing real fits in one file | M | P0 |
| 5.2 | Package manager and manifest (`om.toml`, `om add`, lockfile, git and path dependencies first, registry later) | No ecosystem without it; lockfile also serves 3.12 | L | P0 |
| 5.3 | C FFI (`extern "C"`) and a binding generator from C headers | The design's bridge to every existing library (SQLite, OpenSSL, libc) | M | P0 |
| 5.4 | Standard library: files, directories, processes, env, time, random, JSON, command-line args, stdin lines and keys | Hollow Bell had to add input from scratch; JSON and args are table stakes for CLI tools | L | P0 |
| 5.5 | Trait objects `dyn Trait`, generic traits (`From[T]`), associated types | Needed for plugin-style code, error types and iterator traits | M | P1 |
| 5.6 | Structured concurrency, threads and channels (with 3.11), then async I/O for the HTTP library | Phase 6's server story depends on it | L | P1 |
| 5.7 | Platform support: Windows and macOS on CI with release binaries, cross-compilation, WebAssembly target | `docs/windows.md` exists; a language needs one-command install everywhere | M | P1 |
| 5.8 | Editor support: VS Code extension and tree-sitter grammar (syntax highlighting on GitHub too) | Humans read the code agents write; tree-sitter also gives agents a parser | S | P1 |
| 5.9 | A versioning and stability policy: semver for the language, editions for breaking syntax changes, `om fix --edition` migrations | Users and models need to know what will keep compiling | S | P1 |
| 5.10 | Debugger support: DWARF debug info so `gdb`/`lldb` work, readable panics with source lines | Cranelift supports debug info; without it, crashes are opaque | M | P1 |
| 5.11 | Interop with Rust crates (call `extern "C"` Rust code, or generate bindings) | *(speculative)* Could borrow Rust's ecosystem directly; hard in general, easy for C-ABI crates | L | P2 |
| 5.12 | Self-hosting the compiler | A credibility milestone and the largest Om test case, but late and optional as design.md says | L | P2 |
| 5.13 | Split `om-codegen` (6,000 lines doing checking and codegen) into `om-check` and `om-codegen` as architecture.md plans | Every item above touches the checker; contributor velocity depends on this | M | P0 |
| 5.14 | A real-world showcase beyond games: a CLI tool and a small HTTP service, written by an agent with the notes kept like the game notes | Finds the next set of pain points and gives users proof Om works for normal software | M | P1 |

---

## Suggested order

1. **Now (P0, small):** 0.1, 0.2, 0.4, 0.8, 0.9, 1.2, 2.2, 4.1, 5.13. Each is
   days of work and fixes a measured failure or measurement gap.
2. **Next (P0, larger):** closures, iterators, `HashMap` (0.5 to 0.7), integer
   inference (0.3), modules (5.1), fixed arrays (4.7, 0.12), Rust-shaped
   spellings (1.1), faster `sort`/strings (4.5), better fixes (1.9). Then re-run
   all three benchmarks.
3. **Then:** `om-ir` plus LLVM (4.2, 4.3, 4.4), LSP (1.3), package manager and
   FFI (5.2, 5.3), stdlib (5.4), borrow checker completion (3.1).
4. **After that:** capabilities and sandboxing (3.4 to 3.6), concurrency, regions,
   constrained decoding (1.5, 1.6), verification experiments.

## What would count as "viable"

Targets to check each release, so the roadmap has a finish line:

- **AI usage:** Om pass@1 within 5 points of Rust on a 50+ task suite for at
  least two models, and equal to Rust after one repair round with `om fix`.
- **Tokens:** model-written Om at or below 0.90x Rust tokens on Claude's
  tokenizer, and total session tokens (guide included) at or below Rust's.
- **Safety:** no known soundness holes in safe Om; fuzzing and sanitizers in CI;
  capabilities enforced for packages.
- **Speed:** geometric mean within 1.2x of `rustc -O` in release mode, and at
  least one published benchmark (allocation-heavy, using regions) where Om wins.

## Sources

- Om's own [benchmark results](benchmarks/RESULTS.md),
  [game notes](games/emberdeep/NOTES.md) ([and](games/hollow-bell/NOTES.md)),
  [Rust parity table](docs/rust-parity.md) and [design](docs/design.md).
- Dong et al., [XGrammar: flexible and efficient structured generation](https://arxiv.org/pdf/2411.15100) (2024).
- Mündler et al., [Type-constrained code generation with language models](https://arxiv.org/abs/2504.09246) (PLDI 2025).
- Giagnorio et al., [Enhancing code generation for low-resource languages: no silver bullet](https://arxiv.org/html/2501.19085v1) (2025).
- MoonBit team, [The future of programming languages in the era of LLM](https://www.moonbitlang.com/blog/ai-coding) and [MoonBit: an AI-friendly language](https://conf.researchr.org/details/icse-2024/llm4code-2024-papers/9/MoonBit-Explore-the-Design-of-an-AI-Friendly-Programming-Language) (LLM4Code 2024).
- Yang et al., [AutoVerus: automated proof generation for Rust code](https://arxiv.org/html/2409.13082v3).
- Borretti, [Introducing Austral: linear types and capabilities](https://borretti.me/article/introducing-austral).
- [Cranelift vs LLVM discussion](https://bytecodealliance.github.io/zulip-archive/stream/217117-cranelift/topic/Comparisons.20between.20cranelift.20and.20LLVM.3F.html) and the [Rust project goal on cg_clif performance](https://rust-lang.github.io/rust-project-goals/2026/improve-cg_clif-performance.html).
