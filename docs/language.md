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

## Built-in functions

| Function | Effect |
|---|---|
| `print(x)` | Prints an `i64`, `bool` or `str` followed by a newline |

## Program entry

Every program needs `fn main()`. It takes no parameters and returns nothing or
an `i64` exit code.
