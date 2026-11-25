# 5. Structs

A struct gives a name to a group of related values.

## Defining and creating

```om
struct User {
    name: String,
    email: String,
    logins: i64,
    active: bool,
}

fn main() {
    let mut user = User {
        name: String::from("asha"),
        email: String::from("asha@example.com"),
        logins: 1,
        active: true,
    }
    user.logins += 1
    print(user.name)
    print(user.logins)
}
```

```console
$ om run main.om
asha
2
```

Fields are read and set with a dot. To change any field, the whole variable
must be `mut`.

When a variable has the same name as a field, you can write it once:

```om
struct Point {
    x: i64,
    y: i64,
}

fn point(x: i64, y: i64) -> Point {
    Point { x, y }
}

fn main() {
    print(point(3, 4))
}
```

```console
$ om run main.om
Point { x: 3, y: 4 }
```

`print` shows any struct. Rust needs `#[derive(Debug)]` for that; Om doesn't.

## Methods

Functions that belong to a type go in an `impl` block:

```om
struct Rect {
    width: f64,
    height: f64,
}

impl Rect {
    fn new(width: f64, height: f64) -> Self {
        Rect { width, height }
    }

    fn square(size: f64) -> Self {
        Rect { width: size, height: size }
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }

    fn can_hold(&self, other: &Rect) -> bool {
        self.width > other.width && self.height > other.height
    }

    fn grow(&mut self, by: f64) {
        self.width += by
        self.height += by
    }
}

fn main() {
    let mut big = Rect::new(10.0, 8.0)
    let small = Rect::square(3.0)
    print(big.area())
    print(big.can_hold(&small))
    big.grow(1.5)
    print(big)
}
```

```console
$ om run main.om
80
true
Rect { width: 11.5, height: 9.5 }
```

The first parameter decides how a method uses the value:

| First parameter | Meaning | Call it on |
|---|---|---|
| `&self` | reads the value | any `Rect` |
| `&mut self` | changes the value | a `let mut` `Rect` |
| `self` | takes ownership; the value is used up | any `Rect` |
| none | an *associated function*, called as `Rect::new(...)` | the type |

`Self` inside an `impl` means the type being implemented, here `Rect`.

## Structs and ownership

A struct made only of numbers, `bool` and `char` owns nothing, so it is copied
like a number. A struct with a `String`, `Vec` or `Box` inside owns heap memory
and is moved:

```om
struct Pixel {
    x: i64,
    y: i64,
}

struct Label {
    text: String,
}

fn main() {
    let p = Pixel { x: 1, y: 2 }
    let q = p              // copy
    print(p)
    print(q)

    let a = Label { text: String::from("hi") }
    let b = a              // move; `a` can't be used now
    print(b.text)
}
```

```console
$ om run main.om
Pixel { x: 1, y: 2 }
Pixel { x: 1, y: 2 }
hi
```

## Generic structs

A struct can take type parameters in square brackets. More in
[chapter 9](09-generics-traits.md).

```om
struct Pair[T] {
    left: T,
    right: T,
}

fn main() {
    let ints = Pair { left: 1, right: 2 }
    let words = Pair { left: "yes", right: "no" }
    print(ints)
    print(words.right)
}
```

```console
$ om run main.om
Pair { left: 1, right: 2 }
no
```

**Not in Om yet:** tuple structs (`struct Meters(f64)`) and unit structs
(`struct Marker;`). Use a struct with one named field, or an empty one
(`struct Marker {}`).

## Try it

- Add a `perimeter` method to `Rect`.
- Write a `Counter` struct with `fn tick(&mut self)` and `fn value(&self) -> i64`.
- Give `User` an associated function `fn new(name: str, email: str) -> Self`
  that starts with zero logins.

Next: [Enums and pattern matching](06-enums.md).
