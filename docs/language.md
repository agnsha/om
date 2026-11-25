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
| `i8 i16 i32 i64 isize` | Signed integers. `isize` is 64-bit on the 64-bit targets Om supports. |
| `u8 u16 u32 u64 usize` | Unsigned integers. |
| `f32`, `f64` | Floats. Literals need a digit after the dot (`1.0`), or an exponent (`1e9`). |
| `bool` | `true` or `false` |
| `char` | A Unicode scalar value: `'a'`, `'\n'`, `'\u{1F600}'`. |
| `str` | A view of UTF-8 text (a pointer and a length), like Rust's `&str`. String literals are `str`. |
| `String` | Growable text that owns its bytes. |
| `Vec[T]` | A growable list on the heap. `[1, 2, 3]` and `[0; n]` make one. |
| `Box[T]` | One value on the heap; lets a type contain itself. |
| `(A, B, ...)` | A tuple. Read fields with `.0`, `.1`, or destructure with `let (a, b) = t`. |
| `Option[T]`, `Result[T, E]` | Built in, see below. |

Integer literals may use `_` as a separator (`1_000_000`), and can be written
in hex, octal or binary: `0xFF`, `0o17`, `0b1010`. A suffix fixes the type:
`5u8`, `1_000i64`, `2.5f32`.

An unsuffixed literal takes its type from its context, as in Rust:
`let x: u8 = 200`, `f(3)` when `f` takes a `u16`, `x + 1` when `x: u32`. With
no context it is `i64` (or `f64`). Unlike Rust, Om infers this locally, from
the surrounding expression, not from later uses in the function; write
`for i in 0u32..n` or annotate the variable when it matters. A literal that does
not fit its type, such as `let x: u8 = 256`, is an error.

Integer overflow panics, in every build: `+`, `-`, `*`, unary `-`, division
of `MIN` by `-1`, division or remainder by zero, and shifts by at least the
bit width all stop the program with Rust's message, such as `panic: attempt to
add with overflow`. Rust only checks in debug builds; Om never wraps silently.
When wrapping or saturating is what you mean, say so with `wrapping_add`,
`checked_add`, `saturating_add` and friends.

String and char escapes: `\n \t \r \0 \\ \" \' \u{...}`.

`i8::MIN`, `u64::MAX`, `u32::BITS`, `f64::EPSILON`, `f64::INFINITY`, `f64::NAN`
and `char::MAX` are available.

### String interpolation

Any expression can go inside `{...}` in a string literal. Write `{{` and `}}`
for literal braces.

```om
let name = "Om"
print("Hello from {name}! 6 * 7 = {6 * 7}, big: {fib(30) > 1000}")
```

An interpolated string builds a `String` (Rust's `format!`). Where a `str` is
expected, such as `s.push_str("{i};")`, it is a temporary that lives until the
end of the statement. Inside `{...}`, nested string literals are not allowed.

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
for x in &list { print(x) }      // borrow each element
for x in list { take(x) }        // move each element out (list is used up)
for (i, name) in &pairs { ... }  // patterns work too

let found = loop {               // `loop` can return a value with `break`
    i += 1
    if i % 7 == 0 { break i }
}
```

`break` leaves the innermost loop and `continue` skips to its next iteration.
A label lets them target an outer loop:

```om
'rows: for y in 0..h {
    for x in 0..w {
        if grid(x, y) == target { break 'rows }
        if skip(x) { continue 'rows }
    }
}
```

A labelled `loop` can also `break 'label value`. `for` works over any integer
type, and `0..=u8::MAX` ends correctly without overflowing.

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
| `== != < <= > >=` | numbers and `char`; `==` and `!=` also on `bool`; any type with `Eq` / `Ord` |
| `\|` | bitwise or, on integers and `bool` |
| `^` | bitwise xor |
| `&` | bitwise and |
| `<< >>` | shifts; `>>` is arithmetic on signed types and logical on unsigned ones |
| `+ -` | numbers |
| `* / %` | numbers. Integer division by zero stops the program. |
| `as` | casts, see below |
| unary `-` `!` | `-` on signed integers and floats; `!` on `bool` (not) and integers (bitwise not) |

This is the same precedence as Rust. Compound assignment works for all of
them: `+= -= *= /= %= &= |= ^= <<= >>=`.

`|>` (pipeline) binds loosest of all. `x |> f(a, b)` means `f(x, a, b)`, and
`x |> f` means `f(x)`. Chains read left to right:

```om
let result = 3 |> double |> add(4) |> square    // square(add(double(3), 4)) = 100
```

*Why:* Rust needs nested calls or method syntax on a trait. Pipelines are a
favourite in Elixir, F# and Gleam, and JavaScript has a proposal for one.

## Number and char methods

Integers: `pow`, `abs`, `signum`, `rem_euclid`, `count_ones`,
`leading_zeros`, `trailing_zeros`, `is_power_of_two`, `checked_add/sub/mul/div`,
`saturating_add/sub`, `wrapping_add/sub/mul`, plus `min`, `max`, `clamp` and
`cmp` from `Ord`.

Floats: `sqrt`, `floor`, `ceil`, `round`, `trunc`, `fract`, `abs`, `signum`,
`powi`, `powf`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`,
`ln`, `log2`, `log10`, `cbrt`, `hypot`, `is_nan`, `is_finite`,
`is_infinite`, `to_degrees`, `to_radians`.

`char`: `is_ascii_digit`, `is_ascii_alphabetic`, `is_ascii_alphanumeric`,
`is_ascii_whitespace`, `is_ascii_punctuation`, `is_whitespace`, `to_digit`,
`is_digit`, `to_ascii_uppercase`, `to_ascii_lowercase`, `len_utf8`,
`char::from_digit`, `char::from_u32`.

These are written in Om in the prelude. `sqrt`, `floor`, `count_ones` and the
like compile to single machine instructions.

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
(`let x: Option[i64] = None`). Use trait bounds (below) to say what a `T` must support.

## Traits

A trait lists methods a type must provide; methods with a body are defaults.

```om
trait Shape {
    fn area(&self) -> f64
    fn describe(&self) { print("area {self.area()}") }
}

struct Rect { w: f64, h: f64 }
impl Shape for Rect { fn area(&self) -> f64 { self.w * self.h } }

fn total[T: Shape](a: T, b: T) -> f64 { a.area() + b.area() }
fn show[T](x: T) where T: Shape + Default { x.describe() }
```

Traits can be implemented for built-in types (`impl Ord for i64`) and for
generic types, with bounds: `impl[T: Eq] Eq for Wrap[T]`. Inside a generic
function, `T::name()` calls a function through the type parameter, such as
`T::default()`.

### Built-in traits and operators

| Trait | Gives you |
|---|---|
| `Eq` | `==`, `!=` (`eq`, `ne`) |
| `Ord` | `<`, `<=`, `>`, `>=` via `cmp(&self, &Self) -> Ordering`, plus `max`, `min`, `clamp` |
| `Add`, `Sub`, `Mul`, `Div`, `Rem` | `+ - * / %` and their `+=` forms, between two values of the same type |
| `Neg` | unary `-` |
| `Default` | `T::default()` |

`i64` and `f64` implement all of these; `bool` implements `Eq` and `Default`.

## Option, Result and `?`

```om
enum Option[T] { Some(T), None }
enum Result[T, E] { Ok(T), Err(E) }
```

Both are written in Om in the compiler's prelude, and `Some`, `None`, `Ok`
and `Err` can be used without a prefix. Methods: `is_some`, `is_none`,
`unwrap`, `expect`, `unwrap_or`, `ok_or`, `context` on Option, and `is_ok`,
`is_err`, `unwrap`, `expect`, `unwrap_or`, `ok`, `err`, `context` on Result.

A bare `.unwrap()` is a compile error (E0900) except inside `fn test_*`
functions: a panic with no message is hard to debug, and it is the most common
way a "can't happen" turns into an outage. Use `?`, `unwrap_or`, `match`, or
`expect("why this cannot fail")`.

`context(message)` turns an Option or Result into a `Result[T, String]` whose
error says what was being done. Errors gain one layer per call, so the final
message reads like a trail:

```om
fn port(text: str) -> Result[i64, String] {
    text.parse_int().context("parsing port `{text}`")
}

fn load() -> Result[i64, String] {
    port("80x").context("loading settings")
}
// load() is Err("loading settings: parsing port `80x`: invalid digit found in string")
```

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

## Ownership, Vec, String and Box

Om manages memory the way Rust does: every heap value has one owner, and it is
freed when the owner goes out of scope. There is no garbage collector and no
reference counting.

```om
let a = [1, 2, 3]            // Vec[i64]
let b = a                    // the list moves to `b`
print(a)                     // error: use of moved value `a`
let c = b.clone()            // a deep copy; `b` is still usable
```

Types that own nothing (numbers, `bool`, `char`, `str`, and structs, tuples and
enums made only of those) are copied instead of moved, like Rust's `Copy`.
`.clone()` works on every type. A type that implements `Drop` runs its `drop`
method when it goes away, in the same order as Rust: locals in reverse order of
declaration, then fields in order; list elements front to back.

```om
struct File { name: String }
impl Drop for File {
    fn drop(&mut self) { print("closing {self.name}") }
}
```

Borrowing uses the `&` and `&mut` parameters described above. `let r = &x` and
`match &x` borrow `x` without moving it; `x` cannot be changed or moved while
`r` is in scope. A function that takes `&T` can be passed a loop element
`for x in &list` directly.

**`Vec[T]`**: `Vec::new()`, `with_capacity`, `push`, `pop` (returns an
`Option`), `insert`, `remove`, `swap_remove`, `swap`, `truncate`, `clear`,
`reverse`, `len`, `is_empty`, `capacity`, `get`/`first`/`last` (return a clone
in an `Option`), `append`, `extend_from_slice`, `contains`, `dedup` and a
stable `sort`. `v[i]` reads or assigns an element, and an index past the end
panics with Rust's message. `print` shows `[1, 2, 3]`.

**`String` and `str`**: `String::new()`, `String::from("text")`,
`"text".to_string()` (and `to_string()` on any value), `push`, `push_str`,
`pop`, `insert`, `insert_str`, `remove`, `truncate`, `clear`, `len`,
`as_str`, `into_bytes`, `String::from_utf8`. `s + "more"` and `s += "more"`
append. Every `str` method also works on a `String`: `len`, `is_empty`,
`contains`, `starts_with`, `ends_with`, `find`, `rfind`, `trim`,
`trim_start`, `trim_end`, `strip_prefix`, `strip_suffix`, `split_once`,
`split`, `split_whitespace`, `lines`, `chars`, `bytes`, `replace`, `repeat`,
`to_uppercase`, `to_lowercase` (ASCII letters only for now),
`eq_ignore_ascii_case`, `is_char_boundary`, `slice(start, end)` (Rust's
`&s[start..end]`), and `parse_int()` / `parse_float()`, which return a
`Result`. Strings compare with `==` and `<`. Where a function takes `str`, pass
a String as `&s`.

A `str` taken from a String may not outlive it. Om checks this conservatively:
the String is frozen while the view is in scope, and returning a view of a local
String, keeping one in an outer variable or in a `&mut` list, or taking one of a
temporary are errors. Use `.to_string()` to keep a copy instead.

**`Box[T]`**: `Box::new(value)`. `*b` reads or assigns the value, and moves it
out of an owned box. Fields and methods of the value work on the box directly.
A recursive type needs one:

```om
enum List { Cons(i64, Box[List]), Nil }
```

## Panics

`panic("message {x}")`, `assert(cond)` and `assert(cond, "message")` print
`panic: ...` to stderr and exit with code 101, as Rust does. `unwrap` and
`expect` panic the same way, and `Result::expect` adds the error:
`panic: message: error`. `assert_eq(left, right)` panics with both values.

## Effects

A function can declare what it may do with a `uses` clause after its return
type. The effects are `io` (printing) and `panic` (`panic`, `assert`,
`unwrap`, `expect`, and calls that may do these); `uses` with nothing after it
means pure.

```om
fn area(w: i64, h: i64) -> i64 uses { w * h }
fn report(w: i64) uses io { print("width {w}") }
```

The compiler checks declared effects through every call, including calls to
generic and trait methods (E0901). Functions without a `uses` clause are not
checked, so you can add clauses where they matter. `om audit file.om` lists
the effects of every function. Arithmetic overflow is not counted as a panic
effect.

## Built-in functions

| Function | Effect |
|---|---|
| `print(x)` | Prints any value (numbers, strings, tuples, structs, enums), followed by a newline |
| `panic(msg)` | Prints `panic: msg` to stderr and exits with code 101 |
| `assert(cond)`, `assert(cond, msg)` | Panics if `cond` is false |
| `assert_eq(left, right)` | Panics if `left != right`, showing both |

## Program entry

Every program needs `fn main()`. It takes no parameters and returns nothing or
an `i64` exit code.
