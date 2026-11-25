# Reference and docs

The Learn pages teach. The documents below are the precise descriptions. They
live in the Om repository's `docs/` folder, so they always match the compiler
in that same version.

## The language

| Document | Read it when |
|---|---|
| [Language reference](https://github.com/agnsha/om/blob/main/docs/language.md) | You want the exact rules: every type, operator, pattern, method and built-in that the compiler accepts today. |
| [Rust parity checklist](https://github.com/agnsha/om/blob/main/docs/rust-parity.md) | You want to know whether a Rust feature works in Om yet. Each one is marked done, partial or planned. |
| [Om in one page](om-card.md) | You want the whole language at a glance. |
| [Compiler errors](errors.md) | The compiler said no and you want to know why. |

## The standard library

Om's standard library is small for now and partly written in Om itself (the
compiler's `prelude.om`). What exists:

| Area | Where it's described |
|---|---|
| Numbers and `char`: `pow`, `sqrt`, `checked_add`, `is_ascii_digit` and more | [Language reference: number and char methods](https://github.com/agnsha/om/blob/main/docs/language.md#number-and-char-methods) |
| `Option[T]`, `Result[T, E]` | [Language reference: Option, Result and ?](https://github.com/agnsha/om/blob/main/docs/language.md#option-result-and-) |
| `Vec[T]`, `String`, `str`, `Box[T]` | [Language reference: ownership, Vec, String and Box](https://github.com/agnsha/om/blob/main/docs/language.md#ownership-vec-string-and-box) |
| Traits `Eq`, `Ord`, `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`, `Default`, `Drop` | [Language reference: traits](https://github.com/agnsha/om/blob/main/docs/language.md#traits) |
| `print`, `panic`, `assert` | [Language reference: built-in functions](https://github.com/agnsha/om/blob/main/docs/language.md#built-in-functions) |

**Planned:** generated API docs (an `om doc` command is in review),
`HashMap` and other collections, file and network I/O, closures and
iterators.

## Design and internals

| Document | What's in it |
|---|---|
| [Design document](https://github.com/agnsha/om/blob/main/docs/design.md) | Why Om makes the trade-offs it does: second-class references instead of lifetimes, regions for request-scoped memory, Cranelift for dev builds and LLVM for release. |
| [Compiler architecture](https://github.com/agnsha/om/blob/main/docs/architecture.md) | How source becomes an executable: lexer, parser, type checker, Cranelift, linker. |
| [Contributing](https://github.com/agnsha/om/blob/main/CONTRIBUTING.md) | How to build, test and send changes. |

## Planned books

Rust has a shelf of specialised books. Om will grow the same as the language
does. None of these exist yet:

- **The package guide**, once Om has packages and a package manager.
- **The regions guide**, on Om's arena memory for request handlers.
- **The unsafe and C interop guide**, once `unsafe` and `extern` are user-facing.
- **The web services guide**, alongside the planned HTTP library.
