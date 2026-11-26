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
| `while`, `loop`, `for` | 🟡 | `for` over ranges, `for x in &list` and `for x in list`; `for x in &mut list` and general iterators ⬜ |
| `break`, `continue`, `break value` from `loop`, loop labels | ✅ | |
| Compound assignment `+= -= *= /= %=` | ✅ | |
| Integer types `i8`…`i64`, `u8`…`u64`, `isize`, `usize` | ✅ | `i128`/`u128` ⬜ |
| Literal suffixes, hex/octal/binary literals, exponents | ✅ | |
| Literal type inference | 🟡 | From the surrounding expression; Rust also infers from later uses |
| Bitwise operators `& \| ^ << >> !` and their `=` forms | ✅ | |
| Floats `f32`, `f64`, including `%` | ✅ | |
| `bool`, `char` (Unicode, with escapes) | ✅ | |
| `as` casts | ✅ | Same rules as Rust: integer widening and truncation, saturating float→int, `u8 as char`, `char as u32` |
| Overflow checks in debug builds | ✅ | Om checks in every build: `+ - *`, negation, `/` and `%` (by zero and `MIN / -1`) and over-wide shifts panic with Rust's messages; `wrapping_*`, `checked_*`, `saturating_*` opt out |
| `const` items | ✅ | Literal values only; const expressions ⬜ |
| Associated constants on built-in types (`u8::MAX`, `f64::EPSILON`) | ✅ | User-defined associated consts ⬜ |
| Standard number and char methods | 🟡 | The common ones (see the language reference); more over time |
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
| Arrays `[T; N]` and slices | ⬜ | `[a, b]` and `[x; n]` make a `Vec` for now; `str` is Om's string slice |
| `Vec[T]` | ✅ | Written in Om (`prelude.om`): push, pop, insert, remove, swap_remove, truncate, clear, reverse, sort (stable), dedup, contains, append, get/first/last, `v[i]` with Rust's bounds-check panic, `[a, b]` and `[x; n]` literals |
| `String` and `str` | ✅ | `str` is a (pointer, length) view like Rust's `&str`; 40+ methods (find, split, trim, replace, lines, chars, bytes, strip_prefix, split_once, ...); `s + "x"`, `s += "x"`; `to_string()` on any value. `to_uppercase` changes ASCII letters only. Parsing is `parse_int()` / `parse_float()` until generic `parse::<T>()` |
| `Box[T]` | ✅ | `*b` reads, writes and moves out, field access and methods auto-deref |
| `HashMap<K, V>`, `HashSet`, `VecDeque`, `BTreeMap` | ⬜ | Next batch, with iterators |
| Recursive types | ✅ | Through `Box` or `Vec` (`enum List { Cons(i64, Box[List]), Nil }`) |
| Type aliases `type X = Y` | ✅ | Generic aliases `type Pair[T] = (T, T)` ⬜ |

## Pattern matching

| Feature | Status | Notes |
|---|---|---|
| `match` on integers, bools and enums | ✅ | |
| Literal, range (`1..=9`, `0..10`, `'a'..='z'`), wildcard and binding patterns | ✅ | |
| Nested variant patterns | ✅ | |
| Or-patterns `A \| B` | 🟡 | Without bindings |
| Match guards `if cond` | ✅ | |
| Exhaustiveness checking with a missing-case example | ✅ | Same matrix algorithm family as rustc, including integer and char ranges (`0..=127` plus `128..=255` covers `u8`) |
| `if let`, `while let`, `let else` | ✅ | |
| Destructuring in `let` (tuples, nested, `mut` bindings) | ✅ | Refutable `let` patterns are rejected, as in Rust |
| Tuple patterns in `match` | ✅ | |
| Struct patterns, `..` rest patterns | ✅ | `Point { x, y: 0, .. }`; struct-like enum variants and `@` bindings ⬜ |
| String patterns | ⬜ | Compare with `==` for now |

## Ownership and references

| Feature | Status | Notes |
|---|---|---|
| `&T` and `&mut T` parameters | ✅ | Mutable value semantics: no lifetimes needed |
| Aliasing check (`&mut x` twice in one call) | ✅ | |
| Move semantics and use-after-move errors | ✅ | Including moves in one branch, moves inside loops, moving out of borrowed values, and partial moves of a field |
| `Copy` / `Clone` | ✅ | Types that own nothing copy implicitly; `.clone()` works on every type (deep copy) unless it implements `Drop`, which needs its own `Clone` impl |
| `Drop` | ✅ | Runs in Rust's order: locals in reverse, fields in order, list elements front to back, temporaries at the end of the statement; drop flags for values moved on some paths. Checked leak-free with valgrind |
| Borrow checking | 🟡 | `let r = &x`, `match &x` and `str` views of a String freeze the owner while they live; a view may not outlive its String (returning, storing in an outer variable or a `&mut` list, or pointing into a temporary are errors). This is a conservative stand-in for Rust's lifetimes, not the full NLL checker |
| References stored in structs, lifetimes | ✖ by design | Om uses regions instead (see design) |
| Raw pointers and `unsafe` | 🟡 | `Ptr[T]` and memory intrinsics exist for the standard library; user-facing `unsafe` ⬜ |

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
| Built-in tests (`om test`) | ✅ | `fn test_*()`, each in its own process with a timeout; passing results are cached by content hash; `--json` |
| Formatter (`om fmt`) | ✅ | Layout only (indentation, blank lines, trailing space); `--check` for CI |
| `panic`, `assert`, `unwrap`/`expect` | ✅ | Message on stderr, exit code 101 like Rust. Bare `.unwrap()` outside `fn test_*` is a compile error (E0900), a deliberate difference that makes failure handling explicit; use `expect`, `?` or `unwrap_or` |
| `assert_eq` | ✅ | A function, not a macro; prints both values like Rust |
| Compiler diagnostics | ✅ | Every message has a stable code (`om explain E0200`), all errors in one run, machine-applicable fixes (`om fix`), `--brief` and `--json` output |
| Docs from the command line | ✅ | `om doc Vec`, `om doc Option.unwrap_or`, `om doc` lists everything |
| Threads, `Send`/`Sync`, async | ⬜ | Om plans structured concurrency |
| C FFI (`extern`) | ⬜ | |

## Extras Om has that Rust doesn't

Default and named arguments, the `|>` pipeline operator, interpolation of any
expression in strings, `defer`, printing any value without `derive`, checked
effects (`uses io`, `uses panic`) with `om audit`, `context()` error trails,
and tooling built for AI agents (`om guide`, `om fix`, brief and JSON errors).
See the [language reference](language.md).
