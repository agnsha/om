# 6. Enums and pattern matching

A struct is "this *and* that". An enum is "this *or* that": a value that is
exactly one of several variants.

## Defining an enum

```om
enum Direction {
    North,
    East,
    South,
    West,
}

fn turn_right(d: Direction) -> Direction {
    match d {
        North => Direction::East,
        East => Direction::South,
        South => Direction::West,
        West => Direction::North,
    }
}

fn main() {
    let d = Direction::North
    print(turn_right(d))
    print(turn_right(turn_right(d)))
}
```

```console
$ om run main.om
East
South
```

To make a value you write the full name, `Direction::North`. In the patterns
on the left of a `match` on a `Direction`, the short name `North` is enough.

## Variants with data

Each variant can carry its own values:

```om
enum Message {
    Quit,
    Move(i64, i64),
    Write(String),
    Color(u8, u8, u8),
}

fn handle(m: Message) {
    match m {
        Quit => print("quitting"),
        Move(x, y) => print("moving to {x}, {y}"),
        Write(text) => print("text: {text}"),
        Color(r, g, b) => print("color #{r} {g} {b}"),
    }
}

fn main() {
    handle(Message::Move(3, 4))
    handle(Message::Write(String::from("hello")))
    handle(Message::Color(255, 128, 0))
    handle(Message::Quit)
}
```

```console
$ om run main.om
moving to 3, 4
text: hello
color #255 128 0
quitting
```

Patterns like `Move(x, y)` both check the variant and pull out its values.

## match

`match` compares a value against patterns from top to bottom and runs the first
arm that fits. It's an expression, so it produces a value.

```om
fn describe(n: i64) -> str {
    match n {
        0 => "zero",
        1 | 2 | 3 => "a few",
        4..=9 => "several",
        x if x < 0 => "negative",
        _ => "lots",
    }
}

fn main() {
    for n in [0, 2, 7, -5, 100] {
        print("{n}: {describe(n)}")
    }
}
```

```console
$ om run main.om
0: zero
2: a few
7: several
-5: negative
100: lots
```

| Pattern | Matches |
|---|---|
| `0`, `true`, `'a'` | that exact value |
| `1 \| 2 \| 3` | any of them |
| `4..=9`, `0..10`, `'a'..='z'` | a range |
| `x` | anything, and names it `x` |
| `_` | anything, without naming it |
| `x if x < 0` | anything for which the guard is true |
| `Move(x, _)` | a variant, with patterns for its values |
| `(0, y)` | a tuple, with patterns for each part |

### Every case must be covered

If a value could slip through every arm, the program doesn't compile, and the
compiler tells you a value you missed:

```om
enum Light {
    Red,
    Yellow,
    Green,
}

fn main() {
    let l = Light::Red
    match l {
        Red => print("stop"),
        Green => print("go"),
    }
}
```

```text
error: `match` does not cover every case; missing: Light::Yellow (add an arm for it or a `_` arm)
```

This is one of the most useful checks in the language. Add a variant to an
enum, and the compiler lists every `match` that needs updating.

### Matching tuples

```om
fn fizz(n: i64) -> str {
    match (n % 3 == 0, n % 5 == 0) {
        (true, true) => "FizzBuzz",
        (true, false) => "Fizz",
        (false, true) => "Buzz",
        (false, false) => "",
    }
}

fn main() {
    print(fizz(15))
    print(fizz(9))
}
```

```console
$ om run main.om
FizzBuzz
Fizz
```

## Methods on enums

Enums can have `impl` blocks, just like structs. Matching on `self` is common:

```om
enum Shape {
    Circle(f64),
    Rect(f64, f64),
}

impl Shape {
    fn area(&self) -> f64 {
        match self {
            Circle(r) => 3.14159 * r * r,
            Rect(w, h) => w * h,
        }
    }
}

fn main() {
    let shapes = [Shape::Circle(1.0), Shape::Rect(2.0, 5.0)]
    for s in &shapes {
        print(s.area())
    }
}
```

```console
$ om run main.om
3.14159
10
```

## if let

When you care about one variant only, `if let` is shorter than a `match`:

```om
enum Reply {
    Text(String),
    Empty,
}

fn main() {
    let r = Reply::Text(String::from("pong"))
    if let Text(t) = r {
        print("got {t}")
    } else {
        print("nothing")
    }
}
```

```console
$ om run main.om
got pong
```

`while let` repeats as long as the pattern matches. You'll see it with
`Option` in the next chapter.

## Turning simple enums into numbers

An enum whose variants carry no data can be cast to an integer with `as`:

```om
enum Level {
    Low,
    Medium,
    High,
}

fn main() {
    print(Level::High as i64)
}
```

```console
$ om run main.om
2
```

**Not in Om yet:** struct-like variants (`Move { x: i64, y: i64 }`), struct
patterns, `..` rest patterns and `@` bindings.

## Try it

- Add a `Triangle(f64, f64)` variant (base and height) to `Shape`, then run
  `om check` and see what the compiler asks for.
- Write `enum Coin { Penny, Nickel, Dime, Quarter }` and a function that returns
  each coin's value in cents.

Next: [Handling errors](07-errors.md).
