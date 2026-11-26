You are writing small programs for a benchmark. Write each task below as one
complete, runnable Om program.

Rules:
- A single `.om` file with `fn main()`. Om is a new language: its whole guide is below.
- Each program has no input and must print exactly the lines the task asks for.
- Write idiomatic, normal code, the way you would for a colleague.
- Do not use any tools, do not read or write files, and do not run anything.
  Answer from what you know and from the Om guide below.
- Reply with only the programs: for each task, a line `### <task id>` followed
  by one fenced code block. No other text.

# The Om guide

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

- Generics use `[]`: `Option[i64]`, `Vec[String]`, `fn first[T: Ord](a: T)`, `impl[T] Stack[T]`. Never `<>`.
- No macros: `print("x = {x}, sum = {a + b}")` (any expression in `{}`), `panic("...")`, `assert(c)`, `assert_eq(a, b)`. `"..{x}.."` builds a `String` (Rust's `format!`). `[1, 2, 3]` and `[0; n]` make a `Vec`.
- No `pub`, `use`, `mod`, `unsafe`, lifetimes, `#[derive]`, closures or iterators (yet).
- Ownership as in Rust: `String`, `Vec[T]`, `Box[T]` (and types holding them) move on assignment and are dropped automatically; `.clone()` deep-copies. Types that own nothing copy implicitly.
- `str` is Rust's `&str` and string literals are `str`. Make a String with `"a".to_string()` or `String::from("a")`; pass a String where `str` is wanted as `&s`.
- `&T` / `&mut T` appear on parameters, where the call site repeats them (`bump(&mut n)`), in `let r = &x` and in `for x in &list`. No `&` in struct fields or return types.
- Semicolons are optional. The last expression of a block is its value.
- Integer overflow, division by zero, over-wide shifts and `v[i]` out of range panic (in every build). Use `wrapping_add`, `checked_add`, `saturating_add` or `v.get(i)` when you mean it.
- Bare `x.unwrap()` is a compile error outside `test_*` functions (E0900). Use `?`, `unwrap_or`, `match`, `expect("why it cannot fail")`, or `.context("what failed")?` to turn an Option or Result into `Result[T, String]` with a readable error trail.
- `print` takes one argument. Every value prints without any trait.
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

fn parse_age(s: str) -> Result[i64, String] {
    let n = s.trim().parse_int().context("age `{s}`")?
    if n < 0 { return Err("age {n} is negative") }
    Ok(n)
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
    let mut names: Vec[String] = Vec::new()
    names.push("ada".to_string())
    let copy = names.clone()
    for n in &copy { print("hi {n}, {n.len()} letters") }
    print(names[0].to_uppercase())
    match parse_age("x") { Ok(a) => print(a), Err(e) => print(e) }
}
```

- Types: `i8 i16 i32 i64 isize u8 u16 u32 u64 usize f32 f64 bool char str String`, `Vec[T]`, `Box[T]`, tuples `(A, B)` (fields `.0`), `Option[T]` (`Some`, `None`), `Result[T, E]` (`Ok`, `Err`), `Ordering`. `om doc Vec` and `om doc String` list their methods.
- Literals: `1_000`, `0xFF`, `0b1010`, `5u8`, `2.5f32`, `'a'`, `"\n"`, `"{{literal braces}}"`.
- Control flow: `if`/`else` and `match` are expressions; `while`, `for i in a..b` (`..=` inclusive), `loop` with `break value`, labels `'outer: for ...` with `break 'outer`, `if let`, `while let`, `defer expr` (runs at block exit, newest first).
- Patterns: `_`, names, literals, ranges `1..=9`, `A | B`, variants, tuples, guards `x if x > 0`. `match` must be exhaustive.
- Calls: named arguments `f(1, height: 2)`, defaults `fn f(x: i64 = 1)`, pipeline `x |> f(a)` means `f(x, a)`.
- Traits in the prelude: `Eq` (`==`), `Ord` (`<`, `max`, `min`, `clamp`), `Add Sub Mul Div Rem Neg` (operators), `Default`, `Clone`, `Drop`.
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
- E10xx ownership: E1000 use after move, E1001 move out of a borrow, E1002 borrow conflict, E1003 `str` outlives its String


# Tasks

## 01_fizzbuzz
For each number from 1 to 15, print `FizzBuzz` if it is divisible by 15, `Fizz`
if divisible by 3, `Buzz` if divisible by 5, and the number otherwise. One per line.

## 02_word_count
Count the words in the text `the quick brown fox jumps over the lazy dog the fox`
(split on spaces). Print each distinct word once, in order of first appearance,
as `word: count`.

## 03_bank
Write an `Account` type with an owner name and an integer balance, a `deposit`
method, and a `withdraw` method that fails with the error message
`insufficient funds` instead of going below zero. Starting from an account for
`ada` with balance 100, perform: deposit 50, withdraw 30, withdraw 500, deposit 5.
After each step print `ok <balance>` or `error: <message>`. Finally print
`ada has <balance>`.

## 04_shapes
Define shapes as an enum: `Square(side)`, `Rect(width, height)` and
`Triangle(base, height)`, all integers. The area of a triangle is
`base * height / 2` with integer division. For the list Square(3), Rect(2, 5),
Triangle(4, 3), Square(1), print each area on its own line, then
`total <sum>`, then `largest <largest area>`.

## 05_stack
Write a generic stack type with `push`, `pop` (returns nothing when empty),
`peek` and `len`. Push 1, 2, 3 onto an integer stack, then print `len 3`,
`peek 3`, and pop until empty, printing `pop <value>` for each, then print
`empty` when pop returns nothing. Then push the strings `a` and `b` onto a
string stack and print `top b`.

## 06_parse_ages
Parse these lines, each `name,age`: `alice,30`, `bob,x`, `carol,25`, `dave`,
`erin,-4`. For a valid line print `<name> is <age>`. For an age that is not an
integer print `bad age for <name>`. For a line without a comma print
`bad line: <line>`. For a negative age print `negative age for <name>`. Finally
print `sum <sum of valid ages>`.

## 07_binary_search
Implement binary search over the sorted list 1, 3, 5, 7, 9, 11, 13, 15, 17, 19.
For each query 7, 1, 19, 4, 20, print `<query> at <index>` if found or
`<query> missing` if not.

## 08_roman
Convert each of 1994, 2024, 4, 9, 3999, 58 to a Roman numeral and print
`<number> = <roman>`.

## 09_brackets
For each of the strings `([]{})`, `([)]`, `((`, `{[()()]}`, `)(`, print
`<string> valid` or `<string> invalid` depending on whether the brackets
(), [] and {} are balanced and properly nested.

## 10_primes
Print the first 10 prime numbers on one line separated by single spaces, then
print `count <n>` where n is the number of primes below 10000.

## 11_expr_tree
Represent arithmetic expressions as a recursive tree type with integer
literals and Add, Sub and Mul nodes. Build the tree for `(2 + 3) * (7 - 4)` and
the tree for `10 - 2 * 3` (usual precedence), then print the value of each, one
per line. Then print the depth of the first tree as `depth <d>`, where a literal
has depth 1.

## 12_caesar
Write `shift(text, k)` that rotates ASCII letters by k places (keeping case and
wrapping around) and leaves other characters alone. Print
`shift("Hello, World!", 3)`, then `shift` of that result by 23 (which undoes
it), then `shift("xyz ABC", 4)`.
