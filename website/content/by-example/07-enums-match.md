# Enums and match

Enums carry data. `match` supports literals, ranges, `|`, guards and tuples, and the compiler rejects a match that misses a case.

```om
// Enums can carry data. `match` must cover every case; the compiler checks.
enum Shape {
    Circle(f64),
    Rect(f64, f64),
    Empty,
}

fn area(s: Shape) -> f64 {
    match s {
        Circle(r) => 3.14159 * r * r,
        Rect(w, h) if w > 0.0 && h > 0.0 => w * h,
        Rect(_, _) => 0.0,
        Empty => 0.0,
    }
}

fn describe(n: i64) -> str {
    match n {
        0 => "zero",
        1..=9 => "a digit",
        10 | 100 | 1000 => "a power of ten",
        x if x < 0 => "negative",
        _ => "something else",
    }
}

fn main() {
    print(area(Shape::Circle(1.0)))
    print(area(Shape::Rect(2.0, 3.0)))
    print(area(Shape::Empty))
    print(Shape::Rect(1.0, 2.0))
    for n in [0, 7, 100, -3, 55] {
        print("{n} is {describe(n)}")
    }
    let pair = (3, -3)
    match pair {
        (0, _) => print("starts with zero"),
        (a, b) if a + b == 0 => print("opposites"),
        _ => print("no match"),
    }
}
```

Run it:

```console
$ om run 07-enums-match.om
3.14159
6
0
Rect(1.0, 2.0)
0 is zero
7 is a digit
100 is a power of ten
-3 is negative
55 is something else
opposites
```

The source is in [`07-enums-match.om`](07-enums-match.om). Back to [Om by Example](README.md).
