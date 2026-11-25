# 3. Common concepts

This chapter covers the parts every language has: variables, types, functions,
comments and control flow.

## Variables and mutability

```om
fn main() {
    let x = 5
    print(x)
    let mut y = 10
    y = y + 1
    print(y)
}
```

```console
$ om run main.om
5
11
```

`let` creates a variable that can't change after it's set. If you try,
the compiler stops you:

```text
error: cannot assign twice to `x`; declare it with `let mut x`
```

Add `mut` when a value really needs to change. Most don't, and code is easier
to follow when you can see which ones do.

### Shadowing

A new `let` can reuse a name. The new variable hides the old one from that
point on, and it can even have a different type:

```om
fn main() {
    let spaces = "   "
    let spaces = spaces.len()
    print(spaces)
}
```

```console
$ om run main.om
3
```

### Constants

```om
const SECONDS_PER_HOUR: i64 = 3600

fn main() {
    print(SECONDS_PER_HOUR * 24)
}
```

```console
$ om run main.om
86400
```

Constants are declared outside functions, always need a type, and hold a
literal value.

## Types

Om is statically typed: every value's type is known when the program compiles.
Usually Om works it out, and you can always write it down with `: Type`.

| Kind | Types | Examples |
|---|---|---|
| Signed integers | `i8` `i16` `i32` `i64` `isize` | `-5`, `1_000_000`, `0xFF`, `0b1010`, `7i32` |
| Unsigned integers | `u8` `u16` `u32` `u64` `usize` | `255u8`, `0o17` |
| Floats | `f32` `f64` | `3.14`, `1e9`, `2.5f32` |
| Boolean | `bool` | `true`, `false` |
| Character | `char` | `'a'`, `'\n'`, `'\u{1F600}'` |
| Text | `str`, `String` | `"hello"` (see [chapter 8](08-collections.md)) |
| Tuple | `(A, B, ...)` | `(1, true, 'x')` |

A number with no suffix is an `i64` (or `f64` if it has a dot), unless the
code around it needs another type:

```om
fn half(n: u32) -> u32 { n / 2 }

fn main() {
    let a = 10            // i64
    let b: u8 = 200       // the annotation makes 200 a u8
    print(half(9))        // 9 is a u32 here because half takes a u32
    print(a + 1)
    print(b)
    print(u8::MAX)
}
```

```console
$ om run main.om
4
11
200
255
```

Om never converts between number types on its own. Use `as`:

```om
fn main() {
    let count = 7
    let total = 2.0
    print(total / count as f64)
    print(3.99 as i64)
}
```

```console
$ om run main.om
0.2857142857142857
3
```

Tuples group values of different types. Read them with `.0`, `.1`, or unpack
them all at once:

```om
fn main() {
    let point = (3, -2)
    let (x, y) = point
    print(point.0 + point.1)
    print(x * y)
}
```

```console
$ om run main.om
1
-6
```

## Functions

```om
fn square(n: i64) -> i64 {
    n * n
}

fn describe(n: i64) {
    print("{n} squared is {square(n)}")
}

fn main() {
    describe(4)
    describe(12)
}
```

```console
$ om run main.om
4 squared is 16
12 squared is 144
```

Each parameter needs a type. `-> i64` declares what the function returns; leave
it out if the function returns nothing. The last expression in the body is the
return value. Use `return` to leave early:

```om
fn classify(age: i64) -> str {
    if age < 0 {
        return "invalid"
    }
    if age < 18 { "minor" } else { "adult" }
}

fn main() {
    print(classify(-1))
    print(classify(30))
}
```

```console
$ om run main.om
invalid
adult
```

## Comments

```om
// A line comment runs to the end of the line.

/* A block comment
   can span lines. */

fn main() {
    print(1) // after code, too
}
```

## Control flow

### if

`if` needs a `bool`. Om won't treat a number as true or false:

```om
fn main() {
    let n = 7
    if n % 2 == 0 {
        print("even")
    } else if n % 3 == 0 {
        print("divisible by 3")
    } else {
        print("something else")
    }
}
```

```console
$ om run main.om
something else
```

`if` is an expression, so it can produce a value. Both branches must have the
same type:

```om
fn main() {
    let temperature = 31
    let feel = if temperature > 25 { "hot" } else { "fine" }
    print(feel)
}
```

```console
$ om run main.om
hot
```

### Blocks are expressions too

Any `{ }` block produces its last expression:

```om
fn main() {
    let area = {
        let w = 3
        let h = 4
        w * h
    }
    print(area)
}
```

```console
$ om run main.om
12
```

### Loops

`while` repeats while a condition holds:

```om
fn main() {
    let mut n = 3
    while n > 0 {
        print(n)
        n -= 1
    }
    print("liftoff")
}
```

```console
$ om run main.om
3
2
1
liftoff
```

`for` walks a range or a list. `a..b` stops before `b`; `a..=b` includes it:

```om
fn main() {
    for i in 0..3 {
        print(i)
    }
    for name in ["ant", "bee"] {
        print(name)
    }
}
```

```console
$ om run main.om
0
1
2
ant
bee
```

`loop` repeats until you `break`, and `break` can hand back a value:

```om
fn main() {
    let mut tries = 0
    let result = loop {
        tries += 1
        if tries * tries > 50 {
            break tries
        }
    }
    print(result)
}
```

```console
$ om run main.om
8
```

`continue` skips to the next round. With nested loops, give the outer one a
label to break out of both:

```om
fn main() {
    'search: for a in 1..10 {
        for b in 1..10 {
            if a * b == 42 {
                print("{a} x {b}")
                break 'search
            }
        }
    }
}
```

```console
$ om run main.om
6 x 7
```

## Try it

- Write `fn fahrenheit(c: f64) -> f64` and print a table from 0 to 100 °C in
  steps of 10.
- Print the first 20 Fibonacci numbers with a `while` loop.
- Use `loop` and `break value` to find the first number over 1000 that is
  divisible by 7 and 11.

Next: [Ownership](04-ownership.md).
