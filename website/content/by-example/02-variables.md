# Variables and constants

Bindings are immutable unless you write `let mut`. Shadowing and tuple destructuring work like Rust.

```om
// `let` makes an immutable binding; `let mut` makes one you can change.
fn main() {
    let x = 5                 // type inferred: i64
    let y: f64 = 2.5          // with an annotation
    let mut count = 0
    count += 1
    count *= 10
    print("x = {x}, y = {y}, count = {count}")

    // Shadowing: a new `let` with the same name hides the old one.
    let x = x * 2
    print("x is now {x}")

    // Destructuring a tuple.
    let (a, b) = (1, 2)
    print(a + b)

    // Constants are known at compile time.
    print(LIMIT)
}

const LIMIT: i64 = 100
```

Run it:

```console
$ om run 02-variables.om
x = 5, y = 2.5, count = 10
x is now 10
3
100
```

The source is in [`02-variables.om`](02-variables.om). Back to [Om by Example](README.md).
