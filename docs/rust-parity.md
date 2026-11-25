# Rust feature parity

Om's goal is to support everything Rust programmers rely on, sometimes in a
simpler form (see [design.md](design.md) for where Om deliberately differs).
This page tracks progress honestly. ✅ works today and is tested, 🟡 partly
works, ⬜ planned.

## Basics

| Feature | Status | Notes |
|---|---|---|
| Functions, recursion, early `return` | ✅ | |
| `let`, `let mut`, type annotations, shadowing | ✅ | |
| Block expressions, `if`/`else` as expressions | ✅ | |
| `while`, `loop`, `for` | 🟡 | `for` runs over integer ranges (`0..n`, `0..=n`); iterators come with traits |
| `break`, `continue`, `break value` from `loop` | ✅ | Loop labels (`'outer:`) ⬜ |
| Compound assignment `+= -= *= /= %=` | ✅ | |
| Integer type `i64` | ✅ | |
| Other integer types (`i8`…`i128`, `u8`…`u128`, `usize`) | ⬜ | Next batch |
| Floats | 🟡 | `f64` ✅, `f32` ⬜ |
| `bool`, `char` | 🟡 | `bool` ✅, `char` ⬜ |
| `as` casts | 🟡 | i64 ↔ f64, bool → i64, C-like enum → i64 |
| Overflow checks in debug builds | ⬜ | Arithmetic currently wraps |
| `const` items | ✅ | Literal values only; const expressions ⬜ |
| `static` items | ⬜ | |
| Comments `//` and `/* */` | ✅ | Doc comments `///` ⬜ |

## Data types

| Feature | Status | Notes |
|---|---|---|
| Structs with named fields, struct literals, field shorthand | ✅ | |
| Field access and assignment, nested structs | ✅ | |
| Tuple structs, unit structs | ⬜ | |
| Enums with unit and tuple variants | ✅ | Struct-like variants `V { x }` ⬜ |
| `impl` blocks, methods, associated functions, `Self` | ✅ | |
| `self`, `&self`, `&mut self`, `mut self` | ✅ | |
| Tuples `(a, b)`, `.0` indexing, nested tuples | ✅ | |
| Arrays `[T; N]` and slices | ⬜ | Next batch |
| `String`, `Vec<T>`, `HashMap<K, V>`, `Box<T>` | ⬜ | Need the heap and the ownership checker |
| Recursive types | ⬜ | Need `Box`; rejected with a clear error today |
| Type aliases `type X = Y` | ⬜ | |

## Pattern matching

| Feature | Status | Notes |
|---|---|---|
| `match` on integers, bools and enums | ✅ | |
| Literal, range (`1..=9`, `0..10`), wildcard and binding patterns | ✅ | |
| Nested variant patterns | ✅ | |
| Or-patterns `A \| B` | 🟡 | Without bindings |
| Match guards `if cond` | ✅ | |
| Exhaustiveness checking with a missing-case example | ✅ | Same matrix algorithm family as rustc |
| `if let`, `while let` | ✅ | `let else` ⬜ |
| Destructuring in `let` (tuples, nested, `mut` bindings) | ✅ | Refutable `let` patterns are rejected, as in Rust |
| Tuple patterns in `match` | ✅ | |
| Struct patterns, `..` rest patterns, `@` bindings | ⬜ | |
| String patterns | ⬜ | |

## Ownership and references

| Feature | Status | Notes |
|---|---|---|
| `&T` and `&mut T` parameters | ✅ | Mutable value semantics: no lifetimes needed |
| Aliasing check (`&mut x` twice in one call) | ✅ | |
| Move semantics and use-after-move errors | ⬜ | Arrives with heap types (`String`, `Vec`), where it matters |
| `Copy` / `Clone` | ⬜ | Every current type is plain data and copies freely |
| `Drop` | ⬜ | `defer` covers scope cleanup today |
| References stored in structs, lifetimes | ✖ by design | Om uses regions instead (see design) |
| Raw pointers and `unsafe` | ⬜ | For C interop |

## Abstraction

| Feature | Status | Notes |
|---|---|---|
| Generic functions, structs, enums and `impl` blocks | ✅ | Monomorphized like Rust; written `f[T]`, `Pair[A, B]` |
| Type inference for generic calls | 🟡 | From arguments and from the expected type; inference from later uses ⬜ |
| Traits, `impl Trait for Type`, default methods | ✅ | Impls on built-in types (`impl Ord for i64`) and generic types too |
| Trait bounds `[T: A + B]`, `where` clauses, bounds on generic impls | ✅ | Bounds are checked where a generic is used; generic bodies are checked per instantiation, like C++ templates or Zig, rather than against the bounds alone |
| `T::assoc_fn()` calls through a type parameter | ✅ | e.g. `T::default()` |
| Trait objects `dyn Trait` | ⬜ | |
| Associated types and constants | ⬜ | |
| Operator overloading | 🟡 | `Add Sub Mul Div Rem Neg` (same type on both sides), `Eq` for `== !=`, `Ord` for `< <= > >=`; `Index` ⬜ |
| Standard traits `Eq`, `Ord` (with `max`, `min`, `clamp`), `Default`, `Ordering` | ✅ | Om merges `PartialEq`/`Eq` and `PartialOrd`/`Ord` into one trait each |
| Generic traits (`From[T]`), supertraits | ⬜ | |
| `#[derive(...)]` | 🟡 | Printing works on any struct or enum with no derive needed |
| `Option[T]`, `Result[T, E]` and their core methods | ✅ | Written in Om itself (`prelude.om`); more methods arrive with closures |
| `?` operator | ✅ | On Option and Result; error conversion (`From`) ⬜ |
| Closures and `Fn` traits | ⬜ | |
| Iterators and adapters (`map`, `filter`, `collect`) | ⬜ | |
| Macros (`macro_rules!`) | ⬜ | Om plans compile-time functions instead |

## Program structure and tooling

| Feature | Status | Notes |
|---|---|---|
| Modules, `mod`, `use`, `pub` | ⬜ | |
| Packages and a package manager | ⬜ | |
| Built-in tests (`om test`) | ⬜ | |
| Formatter (`om fmt`) | ⬜ | |
| `panic`, `assert`, `unwrap`/`expect` | ✅ | Message on stderr, exit code 101 like Rust |
| Threads, `Send`/`Sync`, async | ⬜ | Om plans structured concurrency |
| C FFI (`extern`) | ⬜ | |

## Extras Om has that Rust doesn't

Default and named arguments, the `|>` pipeline operator, interpolation of any
expression in strings, `defer`, and printing any value without `derive`.
See the [language reference](language.md).
