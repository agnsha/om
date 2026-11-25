<h1 align="center">Om</h1>

<p align="center">
  <b>A systems language aiming for Rust-level speed, faster builds, and no lifetime annotations.</b><br>
  Compiled to native code. No garbage collector. Built for backend services.
</p>

<p align="center">
  <a href="https://github.com/agnsha/om/actions/workflows/ci.yml"><img src="https://github.com/agnsha/om/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue" alt="License"></a>
  <img src="https://img.shields.io/badge/status-phase%200%20(early)-orange" alt="Status">
</p>

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
$ om run examples/hello.om
Hello, Om!
```

> **Status: early and experimental.** Om is in phase 0: the compiler parses a small
> language and produces real native executables through
> [Cranelift](https://cranelift.dev), but ownership, structs, generics and the
> standard library are still ahead. See the [roadmap](#roadmap).

## Why Om

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

You need [Rust](https://rustup.rs) (stable) and a C compiler (`cc`, `gcc` or
`clang`) for linking. Om builds on Linux and macOS (x86-64 and ARM64).

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
om check file.om          # parse and type-check only
```

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

## The language today (phase 0)

- Types: `i64`, `bool`, `str` (string literals)
- `let` and `let mut`, with optional type annotations
- Functions with parameters and return types; recursion
- `if` / `else if` / `else` as expressions, `while`, `return`
- Operators: `+ - * / %`, `== != < <= > >=`, `&& || !` (short-circuit)
- Blocks are expressions; semicolons are optional
- `print(x)` for integers, booleans, strings and interpolated strings
- Default and named arguments, the `|>` pipeline operator, string interpolation and `defer`
- `fn main() -> i64` returns the process exit code

See the [language reference](docs/language.md) and the [`examples/`](examples) folder.

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
| 1 | Core language: more integer types, `for` loops, better errors, `om-check` crate | next |
| 2 | Structs, move semantics, scoped borrows, ownership checker | planned |
| 3 | Enums, pattern matching, generics, interfaces, `Result` and `?` | planned |
| 4 | Standard library (strings, `Vec`, `Map`, files), regions, C interop | planned |
| 5 | LLVM release backend, benchmark suite vs Rust, C and Go | planned |
| 6 | HTTP server library with per-request regions, structured concurrency | planned |

## Contributing

Om is a learning-friendly project and contributions are welcome. Start with
[CONTRIBUTING.md](CONTRIBUTING.md) and issues labelled `good first issue`.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
