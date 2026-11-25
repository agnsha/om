# Learn Om

Om is a compiled systems language with Rust's model of memory (owners, moves,
borrows, no garbage collector) and fewer things to write down: no lifetime
annotations, optional semicolons, default and named arguments, and string
interpolation of any expression. It is early and experimental, and these pages
only describe what the compiler does today. Anything not built yet is marked
**planned**.

## Get started

| | |
|---|---|
| [Install Om](install.md) | Build the `om` command from source on Linux, macOS or Windows (through WSL). |
| [Om on Windows](windows.md) | Native PowerShell setup, no WSL. **In review.** |
| [Getting started](getting-started.md) | Your first program, `om run`, `om build`, and how errors look. Ten minutes. |
| [Om for Rust programmers](om-for-rust.md) | Already know Rust? One page with every difference that matters. |

## Learn the language

| | |
|---|---|
| [The Om Book](book/README.md) | The full tutorial, from "hello" to ownership, traits and generics. Start here if you're new. |
| [Om by Example](by-example/README.md) | 18 short programs, one topic each, with their real output. |
| [Om Exercises](exercises/README.md) | Small broken programs to fix, with hints. Learn by getting the compiler to say yes. |
| [Om in one page](om-card.md) | The whole language on a single page. Handy for a quick refresher, and small enough to paste into an AI assistant. |

## Use Om

| | |
|---|---|
| [Tools](tools.md) | The `om` command: `run`, `build`, `check`, and the tools coming next. |
| [Games](games.md) | Two games written in Om: Emberdeep and The Hollow Bell. **In review.** |
| [Playground](playground.md) | Running Om in the browser. **Planned**; this page says how to get the same loop locally today. |

## Reference

| | |
|---|---|
| [Reference and docs](reference.md) | The language reference, the Rust parity checklist, the design document and compiler internals. |
| [Compiler errors](errors.md) | The errors you'll meet most, what they mean and how to fix them. |

## Where Om is today

Om compiles to real native executables through Cranelift. It already has
structs, enums, `match`, generics, traits, operator overloading, `Option` and
`Result` with `?`, ownership with automatic drop, `Vec`, `String` and `Box`.

Not built yet: closures, iterators, `HashMap`, modules and `pub`, a package
manager, threads and async, and C interop. The project roadmap lists them in
order, and [the Rust parity checklist](https://github.com/agnsha/om/blob/main/docs/rust-parity.md)
tracks every feature as done, partial or planned.
