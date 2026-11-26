# 9. Generics and traits

Generics let you write a function or type once and use it with many types.
Traits describe what a type can do, so generic code can ask for exactly the
abilities it needs.

## Generic functions

Type parameters go in square brackets after the name:

```om
fn first_or[T](list: &Vec[T], fallback: T) -> T {
    if list.len() > 0 { list[0].clone() } else { fallback }
}

fn main() {
    print(first_or(&[3, 4], 0))
    print(first_or(&["x", "y"], "none"))
    let empty: Vec[f64] = Vec::new()
    print(first_or(&empty, 1.5))
}
```

```console
$ om run main.om
3
x
1.5
```

Om works out `T` from the arguments, or from the type you expect back
(`let x: Option[i64] = None`).

Like Rust, Om compiles a separate copy of a generic function for each type it's
used with (this is called monomorphization). Generic code runs exactly as fast
as code written by hand for one type.

## Generic structs, enums and impl blocks

```om
struct Stack[T] {
    items: Vec[T],
}

impl[T] Stack[T] {
    fn new() -> Self {
        Stack { items: Vec::new() }
    }
    fn push(&mut self, x: T) {
        self.items.push(x)
    }
    fn pop(&mut self) -> Option[T] {
        self.items.pop()
    }
    fn size(&self) -> i64 {
        self.items.len() as i64
    }
}

fn main() {
    let mut s: Stack[str] = Stack { items: Vec::new() }
    s.push("a")
    s.push("b")
    print(s.size())
    print(s.pop())
    print(s.pop())
    print(s.pop())
}
```

```console
$ om run main.om
2
Some("b")
Some("a")
None
```

`impl[T] Stack[T]` means "for every type `T`, here are methods on `Stack[T]`".

**A current limit:** `let s: Stack[str] = Stack::new()` doesn't compile yet,
because Om can't work out `T` for an associated function that takes no
arguments from the variable's annotation. Write the struct literal, as above,
until that's fixed. (Built-in types like `Vec::new()` don't have this problem.)
`Option[T]` and `Result[T, E]` are generic enums defined exactly this way.

## Traits

A trait is a list of methods a type promises to have. Methods with a body are
defaults, which a type can keep or replace.

```om
trait Describe {
    fn name(&self) -> String
    fn describe(&self) {
        print("This is {self.name()}.")
    }
}

struct Planet {
    name: String,
    moons: i64,
}

struct Star {
    name: String,
}

impl Describe for Planet {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn describe(&self) {
        print("{self.name} has {self.moons} moons.")
    }
}

impl Describe for Star {
    fn name(&self) -> String {
        "the star {self.name}"
    }
}

fn main() {
    let earth = Planet { name: String::from("Earth"), moons: 1 }
    let sun = Star { name: String::from("Sol") }
    earth.describe()
    sun.describe()
}
```

```console
$ om run main.om
Earth has 1 moons.
This is the star Sol.
```

## Trait bounds

A bound says what a generic type must be able to do. Without one, the body
can't call `describe` on a `T`:

```om
trait Describe {
    fn describe(&self) -> String
}

struct Cat {}
struct Dog {}

impl Describe for Cat {
    fn describe(&self) -> String { "a cat".to_string() }
}
impl Describe for Dog {
    fn describe(&self) -> String { "a dog".to_string() }
}

fn introduce[T: Describe](x: T) {
    print("Meet {x.describe()}.")
}

fn main() {
    introduce(Cat {})
    introduce(Dog {})
}
```

```console
$ om run main.om
Meet a cat.
Meet a dog.
```

Several bounds join with `+`: `[T: Describe + Default]`. For long lists, use
a `where` clause:

```om
fn show[T](x: T) where T: Describe + Default {
}
```

## Built-in traits

These come with the language. Implement them and the matching syntax works on
your type.

| Trait | You implement | You get |
|---|---|---|
| `Eq` | `fn eq(&self, other: &Self) -> bool` | `==` and `!=` |
| `Ord` | `fn cmp(&self, other: &Self) -> Ordering` | `<` `<=` `>` `>=`, `max`, `min`, `clamp`, `sort` |
| `Add` `Sub` `Mul` `Div` `Rem` | `fn add(self, other: Self) -> Self` and so on | `+ - * / %` and `+=` and friends |
| `Neg` | `fn neg(self) -> Self` | unary `-` |
| `Default` | `fn default() -> Self` | `T::default()` |
| `Drop` | `fn drop(&mut self)` | cleanup at end of scope |

```om
struct Money {
    cents: i64,
}

impl Add for Money {
    fn add(self, other: Money) -> Money {
        Money { cents: self.cents + other.cents }
    }
}

impl Eq for Money {
    fn eq(&self, other: &Money) -> bool {
        self.cents == other.cents
    }
}

impl Ord for Money {
    fn cmp(&self, other: &Money) -> Ordering {
        self.cents.cmp(&other.cents)
    }
}

fn main() {
    let a = Money { cents: 250 }
    let b = Money { cents: 175 }
    let total = a + b
    print(total)
    print(a > b)
    print(total == Money { cents: 425 })
    print(a.max(b))
}
```

```console
$ om run main.om
Money { cents: 425 }
true
true
Money { cents: 250 }
```

Om folds Rust's `PartialEq`/`Eq` into one trait, and `PartialOrd`/`Ord` into
another.

## Calling through a type parameter

`T::name()` calls an associated function of whatever type `T` turns out to be:

```om
struct Config {
    retries: i64,
}

impl Default for Config {
    fn default() -> Config {
        Config { retries: 3 }
    }
}

fn make[T: Default]() -> T {
    T::default()
}

fn main() {
    let c: Config = make()
    print(c)
}
```

```console
$ om run main.om
Config { retries: 3 }
```

## How Om checks generic code

Rust checks a generic body once, against its bounds alone. Om checks it each
time it's used with a concrete type, like C++ templates or Zig. Bounds are
still checked at every call, so `introduce(5)` above is an error because `i64`
doesn't implement `Describe`. The difference only shows when a body uses
something its bounds don't mention.

**Not in Om yet:** trait objects (`dyn Trait`), associated types and constants
on traits, generic traits like `From[T]`, and supertraits.

## Try it

- Write `fn largest[T: Ord](list: &Vec[T]) -> T` and try it on numbers and on
  `Money`.
- Implement `Sub` for `Money`.
- Write a generic `struct Queue[T]` with `push` and `pop` that takes from the
  front.

Next: [Om's extras](10-extras.md).
