# Om vs Rust vs Python

A small, reproducible benchmark on three questions:

1. **Speed.** How fast is compiled Om next to Rust and Python?
2. **Tokens.** How many LLM tokens does the same program cost?
3. **Bug rate.** When a model writes the same tasks in each language, how often
   does the program fail to build or print the wrong thing?

**Results: [RESULTS.md](RESULTS.md).** They are reported as measured,
including where Om loses.

## Layout

| Folder | What it holds |
|---|---|
| `speed/` | Seven programs (`fib`, `sieve`, `collatz`, `matmul`, `sort`, `strings`, `nbody`), each written the same way in `.om`, `.rs` and `.py`, and `run.py`, which builds and times them and checks all three print the same answer. |
| `tokens/` | `count.mjs`, which counts tokens with the real `o200k_base` (GPT-4o) and `cl100k_base` (GPT-4) tokenizers. |
| `bugrate/` | `tasks.md` (12 tasks), `tasks/*.expected`, the exact prompts in `prompts/`, the models' raw replies in `responses/`, the extracted programs in `attempts/`, and `run.py`, which builds and runs them. |
| `report.py` | Turns the three `results.json` files into `RESULTS.md`. |

## Run it

You need Rust, Python 3.10+, Node 18+ and a C compiler.

```sh
cargo build --release -p omc          # the om compiler, used from target/release
python3 benchmarks/speed/run.py       # about a minute, most of it Python
python3 benchmarks/bugrate/run.py     # builds and runs every attempt
(cd benchmarks/tokens && npm install && node count.mjs)
python3 benchmarks/report.py          # writes RESULTS.md
```

`python3 benchmarks/speed/run.py --quick` is a fast smoke test with tiny inputs.

## Method, and what to keep in mind

**Speed.** Om is built with `om build`, which today always uses Cranelift, a
code generator tuned for fast compiles rather than fast code. Rust is built with
`rustc -C opt-level=3` (LLVM), and again with `-C overflow-checks=on`, because
Om checks integer overflow and bounds in every build while release Rust does
not check overflow. Times are wall clock, median of 5 runs (Python 1 run),
including process start. Inputs come from the command line so no compiler can
fold the answer at compile time. The programs are written the same way in all
three languages (plain loops and arrays) rather than each language's fastest
style, so this compares code generators, not tricks. Om's planned LLVM release
backend (roadmap phase 5) is not built yet; expect these numbers to change
when it is.

**Tokens.** Three sets are counted: the seven speed programs and the five
`bench/tokens` programs, both written by hand by Om's authors, and the programs
the models wrote in the bug-rate test, counting only tasks where that model's
Om, Rust and Python versions all passed. The hand-written sets may flatter Om,
since its authors wrote them; the model-written set is the fairer one. A model
also needs Om's guide in context once per session, and that cost is reported
separately. Claude's tokenizer is not public, so it is not used.

**Bug rate.** Two models, Claude Sonnet and Claude Haiku as served by Claude Code in November 2026, each got the same task text for each language, with no
tools and no compiler feedback (one shot, pass@1). The Om prompt also includes
the full Om guide (`docs/guide.md`), because no model has trained on Om. A task
passes if it builds, runs without error and prints exactly the expected
output. With 12 tasks per language, one failure is 8 percentage points, so
treat small differences as noise. The prompts, replies and extracted programs
are all committed so anyone can check them.

**Machine.** The committed results come from a 4-core cloud VM (Linux x86-64).
Absolute times will differ on your machine; the ratios should be close.
