# Om guide for agents

Om is a compiled systems language with Rust-like semantics and lighter syntax. No model has trained on it: read this once per session, and use `om doc NAME` instead of guessing an API. A program is one `.om` file with `fn main()`.

## Workflow

```text
om check app.om --brief     # all errors, one line each, with fixes (OM_AGENT=1 makes --brief the default)
om fix app.om               # apply every fix marked auto, re-check, repeat
om explain E0200            # long form of an error code
om doc Option               # real signatures; `om doc Type.method`, `om doc` lists everything
om test app.om              # run fn test_*(); prints only failures; passing results are cached
om run app.om               # build and run;  om build app.om -o app
om fmt app.om               # canonical layout;  om audit app.om  # effects per function
```

Brief errors look like `E0600 app.om:3:5 message` and then `  fix[auto] 2:9 +"mut "`. `auto` fixes are safe, `maybe` ones need a look, `placeholder` ones contain `TODO` for you to fill in. Fix the first error first; later ones may follow from it.

## Differences from Rust (the usual mistakes)

- Generics use `[]`: `Option[i64]`, `fn first[T: Ord](a: T)`, `impl[T] Stack[T]`. Never `<>`.
- No macros: `print("x = {x}, sum = {a + b}")` (any expression in `{}`), `panic("...")`, `assert(c)`, `assert_eq(a, b)`.
- No `pub`, `use`, `mod`, `unsafe`, lifetimes, `#[derive]`, `Box`, `Vec`, `String` (yet). Strings are `str`.
- Every value is plain data: assignment copies, so no `.clone()`, `.to_string()`, `&str`, `&'a`.
- `&T` / `&mut T` exist only on parameters, and the call site repeats them: `bump(&mut n)`.
- Semicolons are optional. The last expression of a block is its value.
- Integer overflow, division by zero and over-wide shifts panic (in every build). Use `wrapping_add`, `checked_add`, `saturating_add` when you mean it.
- Bare `x.unwrap()` is a compile error outside `test_*` functions (E0900). Use `?`, `unwrap_or`, `match`, or `expect("why it cannot fail")`.
- `print` takes one argument. Struct, enum and tuple values print without any trait.
- Numbers never convert implicitly: `x as f64 + 1.5`. Unsuffixed literals take the type their context needs.

## Syntax by example

```om
const LIMIT: i64 = 100

struct Point { x: f64, y: f64 }

impl Point {
    fn new(x: f64, y: f64) -> Self { Point { x, y } }
    fn norm2(&self) -> f64 { self.x * self.x + self.y * self.y }
    fn shift(&mut self, dx: f64) { self.x += dx }
}

enum Shape { Circle(f64), Rect(f64, f64), Empty }

trait Area {
    fn area(&self) -> f64
    fn describe(&self) { print("area {self.area()}") }
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Circle(r) => 3.14 * r * r,
            Rect(w, h) if w > 0.0 => w * h,
            _ => 0.0,
        }
    }
}

fn largest[T: Ord](a: T, b: T) -> T { a.max(b) }

fn area(width: i64, height: i64 = 1) -> i64 { width * height }

fn halve(n: i64) -> Result[i64, str] {
    if n % 2 != 0 { return Err("odd") }
    Ok(n / 2)
}

fn quarter(n: i64) -> Result[i64, str] {
    let h = halve(n)?
    halve(h)
}

fn bump(counter: &mut i64, by: i64) { counter += by }

fn log(n: i64) uses io { print("n = {n}") }

fn main() {
    let mut p = Point::new(3.0, 4.0)
    p.shift(1.0)
    print(p)
    Shape::Rect(2.0, 3.0).describe()
    print(largest(3, 9))
    print(area(5, height: 2))
    let mut total = 0
    for i in 0..LIMIT { if i % 2 == 0 { continue } total += i }
    bump(&mut total, 1)
    log(total)
    let (lo, hi) = (1, 2)
    if let Ok(q) = quarter(8) { print(q + lo + hi) }
    match quarter(6) {
        Ok(q) => print(q),
        Err(e) => print("error: {e}"),
    }
    let found = loop { total -= 1; if total % 7 == 0 { break total } }
    defer print("done")
    print(3 |> largest(10))
    print(found.checked_add(i64::MAX).unwrap_or(-1))
}
```

- Types: `i8 i16 i32 i64 isize u8 u16 u32 u64 usize f32 f64 bool char str`, tuples `(A, B)` (fields `.0`), `Option[T]` (`Some`, `None`), `Result[T, E]` (`Ok`, `Err`), `Ordering`.
- Literals: `1_000`, `0xFF`, `0b1010`, `5u8`, `2.5f32`, `'a'`, `"\n"`, `"{{literal braces}}"`.
- Control flow: `if`/`else` and `match` are expressions; `while`, `for i in a..b` (`..=` inclusive), `loop` with `break value`, labels `'outer: for ...` with `break 'outer`, `if let`, `while let`, `defer expr` (runs at block exit, newest first).
- Patterns: `_`, names, literals, ranges `1..=9`, `A | B`, variants, tuples, guards `x if x > 0`. `match` must be exhaustive.
- Calls: named arguments `f(1, height: 2)`, defaults `fn f(x: i64 = 1)`, pipeline `x |> f(a)` means `f(x, a)`.
- Traits in the prelude: `Eq` (`==`), `Ord` (`<`, `max`, `min`, `clamp`), `Add Sub Mul Div Rem Neg` (operators), `Default`.
- `main` returns nothing or an `i64` exit code. A panic exits with 101.

## Effects

`fn f() uses io` declares that `f` may print; `uses panic` that it may panic (`panic`, `assert`, `unwrap`, `expect`); `uses` alone means pure. Calls are checked through the whole program (E0901). Functions without a `uses` clause are unchecked. `om audit` lists every function's effects.

## Tests

```om
fn add(a: i64, b: i64) -> i64 { a + b }

fn test_add() {
    assert_eq(add(2, 2), 4)
    assert(add(-1, 1) == 0, "inverse")
}

fn main() { print(add(1, 2)) }
```

Tests are `fn test_*()` with no parameters, each run in its own process with a timeout. `om test --json` prints one object per failure and a summary.

## Error codes

Codes are stable. `om explain CODE` gives the long form; `om guide --codes` lists them all.

- E01xx syntax: E0100 general, E0106 macro call, E0107 attribute, E0108 nesting over 256, E0109 Rust keyword
- E02xx names: E0200 variable, E0201 function, E0202 type, E0204 trait, E0205 method, E0206 field, E0207 variant
- E03xx types: E0300 operand types, E0301 mismatch, E0302 return type, E0306 cannot infer, E0307 `?`, E0310 bound
- E04xx calls, E05xx patterns (E0500 non-exhaustive match), E06xx mutability (E0600 needs `let mut`, E0603 needs `&mut`)
- E07xx items, E08xx control flow, E0900 bare unwrap, E0901 undeclared effect, W0100 unchecked generic
