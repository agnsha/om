# Language reference (phase 0)

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
| `i64` | 64-bit signed integer. Arithmetic wraps on overflow in phase 0. |
| `bool` | `true` or `false` |
| `str` | A string literal. Strings can be stored, passed and printed, not yet built at runtime. |

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

## Control flow

`if` is an expression. Both branches must have the same type, unless one of
them always returns.

```om
let size = if n > 100 { "big" } else if n > 10 { "medium" } else { "small" }
```

`while` loops while its condition is true:

```om
let mut i = 0
while i < 10 {
    i = i + 1
}
```

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

## Built-in functions

| Function | Effect |
|---|---|
| `print(x)` | Prints an `i64`, `bool`, `str` or interpolated string, followed by a newline |

## Program entry

Every program needs `fn main()`. It takes no parameters and returns nothing or
an `i64` exit code.
