<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/om-logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/om-logo-light.svg">
    <img alt="The Om programming language" src="assets/om-logo-light.svg" width="50%">
  </picture>

<p><b>A systems language aiming for Rust-level speed, faster builds, and no lifetime annotations.</b></p>

[Getting started](docs/getting-started.md) | [Language reference](docs/language.md) | [Examples](examples) | [Design](docs/design.md) | [Roadmap](#roadmap) | [Contributing](CONTRIBUTING.md)

[![CI](https://github.com/agnsha/om/actions/workflows/ci.yml/badge.svg)](https://github.com/agnsha/om/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue)](#license)
![Status](https://img.shields.io/badge/status-phase%203%20(early)-orange)

</div>

This is the main source code repository for Om. It contains the compiler, the
`om` command line tool, examples, and documentation.

## Why Om?

- **Performance:** Om compiles to native code with no garbage collector and no
  runtime. Generics are monomorphized, and the design rules out hidden copies,
  hidden allocation and exceptions.
- **Simplicity:** Ownership with automatic drop, and references that never need
  lifetime annotations. You get Rust's traits, enums, `match`, `Option`/`Result`
  and `?` with less to write.
- **Productivity:** Default and named arguments, the `|>` pipeline, string
  interpolation and `defer` work today, and errors point at the exact line.

## Quick start

```sh
git clone https://github.com/agnsha/om
cd om
cargo install --path crates/omc   # installs the `om` command
om run examples/hello.om
```

You need [Rust](https://rustup.rs) (stable) and a C compiler for linking. See
[Install](#install) and the [getting started guide](docs/getting-started.md) for details.

## A taste of Om

```om
fn fib(n: i64) -> i64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

fn main() {
    print("Hello, Om!")
    print(fib(20))
}
```

```console
$ om run hello.om
Hello, Om!
6765
```

> **Status: early and experimental.** The compiler produces real native
> executables through [Cranelift](https://cranelift.dev) and already has
> structs, enums, generics, traits, ownership with automatic drop, `Vec`,
> `String` and `Box`. Closures, iterators, modules and most of the standard
> library are still ahead. See the [roadmap](#roadmap) and the
> [Rust parity checklist](docs/rust-parity.md).

## Design goals

Rust proved that a language can be fast and memory-safe without a garbage
collector. Om takes that foundation and makes different trade-offs:

| Goal | How |
|---|---|
| **Match Rust's runtime speed** | Same idea: static types, no GC, monomorphized generics. Release builds will use LLVM, the same optimizer Rust uses. |
| **Compile faster than Rust** | Dev builds use Cranelift, a fast code generator. No lifetimes and a simple trait system keep type checking linear. |
| **No lifetime annotations** | References are *second-class*: you can pass them into functions but not store them in structs. Long-lived data lives in owned values or *regions*. |
| **Fast backend services** | A region (arena) per request: allocations are a pointer bump and the whole request is freed in one step. |
| **No hidden costs** | No implicit copies, no hidden allocation, no exceptions. |

**What Om does *not* claim:** it will not be faster than Rust for general
compute. In release mode both hand similar code to LLVM, so parity is the honest
target. The one place Om aims to beat idiomatic Rust at runtime is
allocation-heavy request handling, where regions are the default instead of an
opt-in crate. All speed numbers are *targets* until the benchmark suite lands in
phase 5, and the results will be published here, losses included.

Read the full [design document](docs/design.md).

## Install

You need [Rust](https://rustup.rs) (stable) and a C compiler for linking:
`cc`, `gcc` or `clang` on Linux and macOS (x86-64 and ARM64), or the Visual
Studio Build Tools on Windows (x86-64). **On Windows, follow the
[Windows guide](docs/windows.md)**: it works straight from PowerShell, no WSL
needed, and covers a prebuilt `om.exe`.

```sh
git clone https://github.com/agnsha/om
cd om
cargo install --path crates/omc   # installs the `om` command
om run examples/hello.om
```

## Usage

```sh
om run   file.om          # compile and run
om build file.om -o app   # compile to a native executable
om check file.om          # every error, with codes and fixes (--brief, --json)
om fix   file.om          # apply the safe fixes until the file is clean
om test  file.om          # run fn test_*(), cached, only failures printed
om fmt   file.om          # canonical layout
om doc   Vec              # real signatures; `om doc` lists everything
om audit file.om          # which functions print or may panic
om watch file.om          # re-check on every save
om guide                  # the whole language on one page, for AI agents
```

### Built for AI agents

Om is designed so a model that has never seen it can write correct code on the
first or second try. `om guide` prints the whole language in under 4,000
tokens. Errors have stable codes and come with fixes marked `auto`, `maybe` or
`placeholder`; `OM_AGENT=1` switches to one line per error:

```text
E0600 app.om:3:5 cannot assign twice to `x`; declare it with `let mut x`
  fix[auto] 2:9 +"mut "
```

`om fix` applies the safe ones, `om doc` answers "what is the real
signature?" instead of guessing, and `om test` prints only what failed.

Errors point at the exact source:

```text
error: cannot assign twice to `x`; declare it with `let mut x`
  --> app.om:3:5
  |
3 |     x = 2
  |     ^^^^^
```

## Features Rust doesn't have

Om borrows popular features from other languages that Rust users keep asking for. All four work today:

```om
// Default and named arguments (Python, Kotlin, Swift). Requested for Rust since 2014 (rust-lang/rfcs#323).
fn area(width: i64, height: i64 = 1, scale: i64 = 1) -> i64 { width * height * scale }
area(5, scale: 3)

// Pipeline operator (Elixir, F#, Gleam). x |> f(y) means f(x, y).
let result = 3 |> double |> add(4) |> square

// String interpolation with any expression (Python f-strings, Kotlin, Swift).
print("6 * 7 = {6 * 7}, fib = {fib(10)}")

// defer (Zig, Go, Swift): cleanup that runs when the block exits, even on early return.
fn process(n: i64) -> i64 {
    print("open {n}")
    defer print("close {n}")
    if n < 0 { return 0 }
    n * 10
}
```

See [`examples/modern.om`](examples/modern.om) and the [language reference](docs/language.md).

## The language today

- Types: every integer type from `i8` to `u64` plus `isize`/`usize`, `f32`, `f64`, `bool`, `char`, `str` (string literals), tuples, structs and enums
- Bitwise operators, hex/binary literals and suffixes, loop labels, and Rust's common number and char methods
- Generic functions, structs, enums and `impl` blocks, monomorphized like Rust
- Traits with default methods, bounds and `where` clauses; operator overloading through `Add`, `Eq`, `Ord` and friends
- `Option[T]` and `Result[T, E]` with `?`, `unwrap`, `expect`, `unwrap_or` and more
- `if let`, `while let`, and destructuring `let (a, b) = ...`
- `panic`, `assert`, and a Rust-style exit code 101 on panic
- Structs, `impl` blocks, methods with `self` / `&self` / `&mut self`
- Enums with data, `match` with guards, ranges, nested patterns and exhaustiveness checking
- `&T` and `&mut T` parameters, with no lifetime annotations
- `for` over ranges, `loop` with `break value`, `continue`, compound assignment, `as` casts, `const`
- `let` and `let mut`, with optional type annotations
- Functions with parameters and return types; recursion
- `if` / `else if` / `else` as expressions, `while`, `return`
- Operators: `+ - * / %`, `== != < <= > >=`, `&& || !` (short-circuit)
- Blocks are expressions; semicolons are optional
- `print(x)` for integers, booleans, strings and interpolated strings
- Default and named arguments, the `|>` pipeline operator, string interpolation and `defer`
- `fn main() -> i64` returns the process exit code

See the [language reference](docs/language.md), the [`examples/`](examples) folder, and the
[Rust parity checklist](docs/rust-parity.md) for what is done and what is next.

## Games written in Om

Two complete terminal games, each with notes on what the language made hard
(most of which has since been fixed in the compiler):

- [Emberdeep](games/emberdeep): a first-person descent by lantern light, drawn
  with a true-colour raycaster, with baked shadows, procedural textures,
  billboard sprites and creatures that hunt you in the dark.
  `om run games/emberdeep/emberdeep.om`
- [The Hollow Bell](games/hollow-bell): a stealth roguelike where sound
  travels through the halls and you fight by composing spoken spells.
  `om run games/hollow-bell/hollow_bell.om`

## How the compiler works

```text
source ─▶ om-lexer ─▶ om-parser ─▶ check + lower ─▶ Cranelift ─▶ object file ─▶ cc ─▶ executable
          tokens      AST          types             native code
```

The compiler is written in Rust and split into small crates. See
[docs/architecture.md](docs/architecture.md).

| Crate | Purpose |
|---|---|
| [`om-lexer`](crates/om-lexer) | Source text to tokens with byte spans |
| [`om-parser`](crates/om-parser) | Recursive-descent parser producing the AST |
| [`om-codegen`](crates/om-codegen) | Type checking and Cranelift code generation |
| [`omc`](crates/omc) | The `om` command line tool, diagnostics and linking |

## Roadmap

| Phase | Focus | Status |
|---|---|---|
| 0 | Lexer, parser, native backend, CLI, tests, CI | ✅ done |
| 0.5 | Default and named arguments, `\|>`, string interpolation, `defer` | ✅ done |
| 1 | Loops, `f64`, structs, methods, enums, `match`, `&`/`&mut` parameters | ✅ done |
| 2 | Generics, `Option`/`Result` and `?`, tuples, `if let`/`while let`, panics | ✅ done |
| 2.5 | Traits, bounds, operator overloading, all integer types, `f32`, `char`, bit operations, loop labels | ✅ done |
| 3 | Memory model: `Vec`, `String`, `Box`, moves, drop, clone, borrow freezing, `for` over lists | ✅ done |
| 3.5 | Closures, iterators, `HashMap`, modules and `pub` | next |
| 4 | Standard library (strings, `Vec`, `Map`, files), regions, C interop | planned |
| 5 | LLVM release backend, benchmark suite vs Rust, C and Go | planned |
| 6 | HTTP server library with per-request regions, structured concurrency | planned |

## Getting help

Om is young, so the issue tracker is the place for questions as well as bugs.
Open an [issue](https://github.com/agnsha/om/issues/new/choose) using the bug
report or language proposal template.

## Contributing

Om is a learning-friendly project and contributions are welcome. Start with
[CONTRIBUTING.md](CONTRIBUTING.md) and issues labelled `good first issue`.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
