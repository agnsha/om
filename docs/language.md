# Language reference

This page describes what the compiler accepts **today**. Features planned for
later phases are in the [design document](design.md).

## Comments

```om
// line comment
/* block comment */
```

## Types

| Type | Meaning |
|---|---|
| `i64` | 64-bit signed integer. Arithmetic currently wraps on overflow. |
| `f64` | 64-bit float. Literals need a digit after the dot: `1.0`, `0.5`. |
| `bool` | `true` or `false` |
| `str` | A string literal. Strings can be stored, passed and printed, not yet built at runtime. |
| `(A, B, ...)` | A tuple. Read fields with `.0`, `.1`, or destructure with `let (a, b) = t`. |
| `Option[T]`, `Result[T, E]` | Built in, see below. |

Integer literals may use `_` as a separator: `1_000_000`.
String escapes: `\n \t \r \0 \\ \"`.

### String interpolation

Any expression can go inside `{...}` in a string literal. Write `{{` and `}}`
for literal braces.

```om
let name = "Om"
print("Hello from {name}! 6 * 7 = {6 * 7}, big: {fib(30) > 1000}")
```

In phase 0, interpolated strings can only be passed to `print`: Om has no
runtime strings until phase 4. Inside `{...}`, nested string literals are not
allowed.

*Why:* Rust's `format!` only accepts plain variable names inline
(`{name}` works, `{a + b}` does not). Python, Kotlin, Swift and JavaScript
allow any expression.

## Variables

```om
let x = 5            // immutable, type inferred
let y: i64 = 10      // with an annotation
let mut count = 0    // mutable
count = count + 1
```

Assigning to a variable declared without `mut` is a compile error. Variables are
scoped to their block, and an inner block may shadow an outer name.

## Functions

```om
fn add(a: i64, b: i64) -> i64 {
    a + b              // last expression is the return value
}

fn greet() {           // no `->` means it returns nothing
    print("hi")
}
```

Parameters must have types. Functions may be declared in any order and may be
recursive. `return` exits early: `return a + b`.

### Default and named arguments

A parameter can have a default value, used when a call leaves it out. Defaults
must be constants (a number, `true`/`false` or a string), and once one
parameter has a default, every later parameter needs one too.

```om
fn area(width: i64, height: i64 = 1, scale: i64 = 1) -> i64 {
    width * height * scale
}

area(5)                       // 5
area(5, 2)                    // 10
area(5, scale: 3)             // skip height, name scale
area(height: 4, width: 2)     // any order when named
```

Positional arguments come first, then named ones. Arguments are evaluated in
the order they are written. Naming an argument that doesn't exist, giving one
twice, or leaving out one with no default is a compile error.

*Why:* Rust has no default or named arguments (requested since 2014), so APIs
fall back to builder types or many function variants.

## Structs and methods

```om
struct Point { x: f64, y: f64 }

impl Point {
    fn new(x: f64, y: f64) -> Self { Point { x, y } }        // associated function
    fn dist2(&self, other: &Point) -> f64 {                   // reads self
        let dx = self.x - other.x
        let dy = self.y - other.y
        dx * dx + dy * dy
    }
    fn shift(&mut self, dx: f64) { self.x += dx }             // changes self
}

let mut p = Point::new(1.0, 2.0)
p.shift(2.0)
p.y = 10.0
print(p)                    // Point { x: 3, y: 10 }
```

Struct values are plain data: assigning one copies it. Any struct or enum can
be printed with `print`, with no `derive` needed.

## Enums and match

```om
enum Shape { Circle(f64), Rect(f64, f64), Empty }

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14159 * r * r,
        Shape::Rect(w, h) if w > 0.0 => w * h,
        Shape::Rect(_, _) => 0.0,
        Shape::Empty => 0.0,
    }
}
```

Patterns: `_`, a name (binds the value), integers, `true`/`false`, ranges
(`1..=9`, `0..10`), variants with nested patterns, and `A | B`. Inside a match
on an enum, variants can be written without the enum name (`Circle(r)`).
Arms may have a guard: `x if x < 0 => ...`.

`match` must cover every case. If it doesn't, the compiler shows an example of
a value that is not handled:

```text
error: `match` does not cover every case; missing: Shape::Rect(_, _)
```

## References: `&T` and `&mut T`

```om
fn bump(counter: &mut i64, by: i64) { counter += by }

let mut c = 10
bump(&mut c, 5)      // c is now 15
```

A `&mut` parameter lets the function change the caller's variable; `&T` gives
read-only access. Unlike Rust, Om references are parameters only: they cannot
be stored in structs or returned. That rule is why Om never needs lifetime
annotations. Internally the value is passed in and its new value handed back
(mutable value semantics, as in Swift's `inout`), so there are no pointers
that could dangle. Passing the same variable as `&mut` twice in one call is
an error.

## Casts

`as` converts between `i64` and `f64` (float to int truncates and saturates,
like Rust), `bool` to `i64`, and enums with only unit variants to `i64`.

## Constants

```om
const LIMIT: i64 = 100
const PI: f64 = 3.14159
```

## Control flow

`if` is an expression. Both branches must have the same type, unless one of
them always returns.

```om
let size = if n > 100 { "big" } else if n > 10 { "medium" } else { "small" }
```

Loops:

```om
let mut i = 0
while i < 10 { i += 1 }

for n in 0..10 { print(n) }      // 0 to 9
for n in 1..=10 { print(n) }     // 1 to 10

let found = loop {               // `loop` can return a value with `break`
    i += 1
    if i % 7 == 0 { break i }
}
```

`break` leaves the innermost loop and `continue` skips to its next iteration.
Compound assignment works on variables and fields: `+= -= *= /= %=`.

Conditions must be `bool`; `if 1 { }` is an error.

## defer

`defer expr` schedules `expr` to run when the enclosing block exits, whether it
ends normally or through `return`. Several defers run newest first.

```om
fn process(n: i64) -> i64 {
    print("open {n}")
    defer print("close {n}")       // always runs, even on the early return
    if n < 0 { return 0 }
    defer print("finished {n}")    // runs before "close"
    n * 10
}
```

Like Zig and Swift, the deferred expression is evaluated when it runs, so it
sees variables' values at the moment the block exits. The return value is
computed before defers run. `return` is not allowed inside a `defer`.

*Why:* Rust runs cleanup through `Drop`, which needs a type per cleanup
action. `defer` (Zig, Go, Swift) puts the cleanup next to the setup.

## Blocks

A block `{ ... }` is an expression. Its value is its last expression when that
expression has no trailing `;`.

```om
let total = {
    let a = 10
    let b = 20
    a + b
}
```

Semicolons between statements are optional.

## Operators

From lowest to highest precedence:

| Operators | Types |
|---|---|
| `\|\|` | bool, short-circuit |
| `&&` | bool, short-circuit |
| `== != < <= > >=` | `==` and `!=` on i64 and bool; ordering on i64 |
| `+ -` | i64 |
| `* / %` | i64. Division by zero stops the program. |
| unary `-` `!` | i64, bool |

`|>` (pipeline) binds loosest of all. `x |> f(a, b)` means `f(x, a, b)`, and
`x |> f` means `f(x)`. Chains read left to right:

```om
let result = 3 |> double |> add(4) |> square    // square(add(double(3), 4)) = 100
```

*Why:* Rust needs nested calls or method syntax on a trait. Pipelines are a
favourite in Elixir, F# and Gleam, and JavaScript has a proposal for one.

## Generics

Type parameters go in square brackets. Om generates a specialised copy for
each set of types used (monomorphization), so generic code runs as fast as
hand-written code, the same as Rust.

```om
struct Pair[A, B] { first: A, second: B }

impl[A, B] Pair[A, B] {
    fn swap(self) -> Pair[B, A] { Pair { first: self.second, second: self.first } }
}

fn pick[T](a: T, b: T, first: bool) -> T { if first { a } else { b } }
```

Type arguments are inferred from the arguments and from the expected type
(`let x: Option[i64] = None`). Trait bounds are not here yet, so a generic
body can only move its `T` values around, not do arithmetic on them.

## Option, Result and `?`

```om
enum Option[T] { Some(T), None }
enum Result[T, E] { Ok(T), Err(E) }
```

Both are written in Om in the compiler's prelude, and `Some`, `None`, `Ok`
and `Err` can be used without a prefix. Methods: `is_some`, `is_none`,
`unwrap`, `expect`, `unwrap_or`, `ok_or` on Option, and `is_ok`, `is_err`,
`unwrap`, `expect`, `unwrap_or`, `ok`, `err` on Result.

`expr?` unwraps a success value or returns the failure from the current
function, which must return the same kind (Option or Result with the same
error type).

```om
fn calc(a: i64, b: i64) -> Result[i64, str] {
    let q = div(a, b)?
    Ok(q * 2)
}
```

## Patterns everywhere

```om
let (lo, hi) = min_max(9, 2)
if let Some(root) = find(49) { print(root) } else { print("none") }
while let Some(n) = stack { ... }
```

A `let` pattern must always match; use `if let` or `match` for patterns that
might fail.

## Panics

`panic("message {x}")`, `assert(cond)` and `assert(cond, "message")` print
`panic: ...` to stderr and exit with code 101, as Rust does. `unwrap` and
`expect` panic the same way.

## Built-in functions

| Function | Effect |
|---|---|
| `print(x)` | Prints any value (numbers, strings, tuples, structs, enums), followed by a newline |
| `panic(msg)` | Prints `panic: msg` to stderr and exits with code 101 |
| `assert(cond)`, `assert(cond, msg)` | Panics if `cond` is false |

## Program entry

Every program needs `fn main()`. It takes no parameters and returns nothing or
an `i64` exit code.
