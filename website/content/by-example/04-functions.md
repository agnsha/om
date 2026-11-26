# Functions

Typed parameters, implicit return of the last expression, early `return`, and Om's default and named arguments.

```om
// Functions take typed parameters. The last expression is the return value.
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn sign(n: i64) -> str {
    if n < 0 { return "negative" }
    if n == 0 { "zero" } else { "positive" }
}

// Default and named arguments (Om extra).
fn greet(name: str, greeting: str = "Hello", times: i64 = 1) {
    for _ in 0..times {
        print("{greeting}, {name}!")
    }
}

fn main() {
    print(add(2, 3))
    print(sign(-4))
    print(sign(0))
    greet("Om")
    greet("Rust", greeting: "Hi")
    greet(times: 2, name: "you")
}
```

Run it:

```console
$ om run 04-functions.om
5
negative
zero
Hello, Om!
Hi, Rust!
Hello, you!
Hello, you!
```

The source is in [`04-functions.om`](04-functions.om). Back to [Om by Example](README.md).
