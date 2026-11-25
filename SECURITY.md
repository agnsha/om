# Security

## Reporting a problem

Please report security problems privately to the maintainers (for example
with GitHub's private vulnerability reporting), not in a public issue.

## What Om protects against

**Compiling untrusted code is safe.** Om has no macros, build scripts,
procedural derives, compile-time file includes or compile-time evaluation, so
`om check`, `om build`, `om fmt`, `om fix` and `om doc` never run code from
the file. Compiling can fail but cannot read your files, open sockets or start
processes (besides the C linker that `om build` runs).

**Hostile source fails fast.** The compiler reads only regular UTF-8 files up
to 16 MiB, caps nesting at 256 levels, types at 1024 scalar values and 64
levels, generic instances at 20,000 and errors at 400 per run, and runs on a
large private stack. `crates/omc/tests/security.rs` fuzzes it with mutated
programs and pathological inputs.

**Programs fail loudly instead of corrupting memory.** Integer overflow,
division by zero, over-wide shifts, out-of-range indexes and string slices,
non-UTF-8 text and impossible allocation sizes all panic with exit code 101,
in every build. Raw memory (`Ptr` and `intrinsic::*`) is only reachable from
the bundled `Vec`, `String` and `Box`; user code that names an intrinsic or a
private field is rejected. Printed text is written as bytes, never used as a
format string.

**The toolchain does not leak or trust shared state.** Intermediate files go in
a fresh directory with a random name and mode 0700, created exclusively and
removed afterwards. Output paths that start with `-` cannot become linker
options. `om fix` and `om fmt` write through a temporary file and rename, so
an interrupted run never leaves half a file. The `om test` cache stores only
SHA-256 hashes of passing tests, in a 0700 directory.

**The repository does not publish itself.** The crates are marked
`publish = false`, the website is deployed only by a manual workflow run
(GitHub Pages sites are public even for private repositories), and CI runs
with read-only permissions, `--locked` dependencies, Dependabot and
`cargo audit`.

## Known limits

- `om build` runs the C compiler named by `$CC` (default `cc`). Only set `CC`
  to a compiler you trust.
- Third-party GitHub Actions are pinned to major versions, not commit hashes.
  Dependabot proposes updates; pinning to hashes is a good next step.
- Borrow checking of `str` views is conservative and checked in tests, but it
  is not Rust's full lifetime analysis.
