# Om for Rust programmers

If you know Rust, you already know most of Om. The memory model is the same:
one owner per value, moves, `.clone()`, `Drop`, and `&` / `&mut` borrows, with
no garbage collector. This page lists only what's different.

## Syntax side by side

| Rust | Om | Note |
|---|---|---|
| `println!("{}", x);` | `print(x)` | `print` takes any value, including structs and enums, without `#[derive(Debug)]`. |
| `format!("{} + {} = {}", a, b, a + b)` | `"{a} + {b} = {a + b}"` | Any expression inside `{}` in any string literal. `{{` and `}}` for literal braces. |
| `let x = 5;` | `let x = 5` | Semicolons are optional. |
| `Vec<T>`, `Option<T>`, `fn f<T>` | `Vec[T]`, `Option[T]`, `fn f[T]` | Square brackets for type parameters. |
| `impl<T> Foo<T>` | `impl[T] Foo[T]` | |
| `vec![1, 2, 3]`, `vec![0; n]` | `[1, 2, 3]`, `[0; n]` | List literals make a `Vec`. Fixed arrays `[T; N]` are planned. |
| `&s[1..4]` | `s.slice(1, 4)` | |
| `s.parse::<i64>()` | `s.parse_int()` / `s.parse_float()` | Until generic `parse` lands. |
| `fn f(s: &str)` | `fn f(s: str)` | `str` is already a view (pointer and length). Pass a `String` as `&s`. |
| `#[derive(PartialEq, PartialOrd)]` | `impl Eq for T`, `impl Ord for T` | `PartialEq`/`Eq` are one trait, and so are `PartialOrd`/`Ord`. |
| `if let Some(x) = o { }` | same | `while let` and destructuring `let (a, b) = t` are the same too. |

## Things Om adds

```om
// Default and named arguments
fn connect(host: str, port: i64 = 80, retries: i64 = 3) { }
connect("example.com", retries: 5)

// Pipeline: x |> f(y) means f(x, y)
let r = 3 |> double |> add(4) |> square

// defer: runs when the block exits, newest first, even on early return
fn work() {
    print("open")
    defer print("close")
}
```

## Things Om leaves out on purpose

**Lifetimes.** References in Om are *second-class*: you can pass `&T` and
`&mut T` into a function, but you can't store a reference in a struct or return
one. That's the whole reason Om never needs `'a` annotations. A `&mut`
parameter works like Swift's `inout`: the function gets the value and hands the
new value back.

Data that would hold references in Rust holds owned values in Om (`String`
instead of `&str`, indexes instead of pointers). For request-scoped data, Om's
planned *regions* will make many small allocations as cheap as a pointer bump.

**Macros.** No `macro_rules!`. Om plans compile-time functions instead.

## Things that work a bit differently

- **Borrow checking.** `let r = &x` and `match &x` freeze `x` while `r` is in
  scope, and a `str` taken from a `String` can't outlive it. This is a simpler,
  more conservative check than Rust's non-lexical lifetimes, so a few programs
  Rust accepts are rejected.
- **Overflow.** Integer arithmetic wraps on overflow in every build. Rust
  panics in debug builds. `checked_add`, `saturating_add` and `wrapping_add`
  exist when you want to be explicit.
- **Literal types.** An unsuffixed literal takes its type from the expression
  around it, not from later uses in the function. Write `0u32..n` or annotate
  the variable when it matters.
- **Generic bodies** are type-checked for each set of types they're used with,
  like C++ templates or Zig, rather than once against the trait bounds.
- **`to_uppercase` / `to_lowercase`** change ASCII letters only, for now.
- **`split`, `split_whitespace`, `lines`, `chars`** return a `Vec` rather than
  an iterator, until iterators exist.

## Not there yet

Closures, iterators and adapters (`map`, `filter`, `collect`), `HashMap` and
the other collections, modules and `pub`, trait objects (`dyn Trait`),
associated types, tuple structs, struct patterns, `i128`, threads and async,
and C FFI. The [parity checklist](https://github.com/agnsha/om/blob/main/docs/rust-parity.md)
has the full list and is updated as each one lands.
