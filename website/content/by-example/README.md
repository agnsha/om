# Om by Example

Short, complete programs, one topic per page. Each page shows the source and
the exact output it prints. Every program here was compiled and run with the
current Om compiler, so what you see is what you get.

Read them in order if you're new, or jump to a topic. Each program also sits in
this folder as an `.om` file, so you can run it with `om run 01-hello.om`.

| # | Topic | What it shows |
|---|---|---|
| 1 | [Hello, world](01-hello.md) | The smallest Om program. |
| 2 | [Variables and constants](02-variables.md) | Bindings are immutable unless you write `let mut`. |
| 3 | [Primitive types](03-primitives.md) | Every integer type from `i8` to `u64`, `isize`/`usize`, `f32`, `f64`, `bool` and `char`, with Rust's limits, number methods and `as` casts. |
| 4 | [Functions](04-functions.md) | Typed parameters, implicit return of the last expression, early `return`, and Om's default and named arguments. |
| 5 | [Control flow](05-control-flow.md) | `if` is an expression. |
| 6 | [Structs and methods](06-structs.md) | Named fields, struct literals with field shorthand, and `impl` blocks with `&self`, `&mut self` and associated functions. |
| 7 | [Enums and match](07-enums-match.md) | Enums carry data. |
| 8 | [Option, Result and ?](08-option-result.md) | No null and no exceptions. |
| 9 | [Vec](09-vec.md) | A growable list that owns its elements. |
| 10 | [String and str](10-strings.md) | `String` owns text and `str` is a view of it. |
| 11 | [Ownership and borrowing](11-ownership.md) | Moves, `.clone()`, `&` and `&mut` parameters. |
| 12 | [Drop](12-drop.md) | Cleanup runs when a value goes out of scope, in Rust's order. |
| 13 | [Box and recursive types](13-box.md) | `Box[T]` stores one value on the heap, which is how a type can contain itself. |
| 14 | [Generics](14-generics.md) | Generic functions, structs and `impl` blocks. |
| 15 | [Traits and operator overloading](15-traits.md) | Traits with default methods, trait bounds, and `+` / `==` through `Add` and `Eq`. |
| 16 | [Pipeline operator](16-pipeline.md) | An Om extra: `x |> f(y)` means `f(x, y)`. |
| 17 | [defer](17-defer.md) | An Om extra from Zig, Go and Swift: put cleanup right next to setup. |
| 18 | [Panics and assert](18-panics.md) | `panic`, `assert` and `unwrap` print to stderr and exit with code 101. |

Want the longer explanations? Read [the Om Book](../book/README.md).
Back to [Learn Om](../README.md).
