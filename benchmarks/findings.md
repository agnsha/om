## What the failures say about Om

Every Om failure was a compile error; no Om program that built printed the
wrong answer. Rust and Python had no failures at all. `om fix --maybe` repaired
none of the five. The causes, each a fixable gap in Om rather than a model
mistake about the task:

1. **`i64` does not satisfy a `Clone` bound** (both models, `05_stack`). `i64`
   has a `.clone()` method but does not count as implementing `Clone`, so a
   generic `impl[T: Clone]` rejects `Stack[i64]`. Rust accepts this.
2. **Braces in a plain string literal are interpolation** (`09_brackets`).
   `"([]{})"` is an error because every string interpolates `{...}`. The guide
   mentions `{{literal braces}}`, and one model used it, but this is a trap
   Rust and Python do not have for ordinary strings. The `maybe` fix that
   `om fix` offers also breaks the other literals on the line.
3. **No integer type inference from later use** (`07_binary_search`). `let mut
   lo = 0` becomes `i64` and then clashes with `usize` from `len()`; Rust infers
   `usize`.
4. **No auto-deref from `Box[T]` to `T`** (`11_expr_tree`). Passing a
   `Box[Expr]` where `&Expr` is wanted fails; Rust derefs it automatically.

On tokens, the hand-written sets put Om below Rust, but in the model-written
programs Om costs slightly *more* than Rust: without iterators, closures or a
`HashMap` (roadmap phase 3.5), models write longer manual loops in Om. Python
is the most compact in every set.

On speed, Om (Cranelift) is between Python and Rust: well ahead of Python on
everything, and 1.1x to 8x behind optimized Rust. The gap is widest where LLVM
vectorizes (`matmul`) and in library code (`sort`, string building), and
smallest on simple loops (`sieve`). Om's overflow checks explain only part of
it. Builds are not faster than `rustc` on programs this small: both take about
0.2 to 0.3 seconds, mostly linking.
