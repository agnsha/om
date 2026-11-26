# 10. Om's extras

Om borrows four popular features from other languages that Rust doesn't have.
Each one removes boilerplate without hiding any cost.

## Default and named arguments

From Python, Kotlin and Swift. A parameter can have a default value, and a
call can name the arguments it passes:

```om
fn connect(host: str, port: i64 = 443, retries: i64 = 3, verbose: bool = false) {
    print("{host}:{port}, {retries} retries, verbose {verbose}")
}

fn main() {
    connect("example.com")
    connect("example.com", 8080)
    connect("example.com", verbose: true)
    connect(retries: 5, host: "localhost")
}
```

```console
$ om run main.om
example.com:443, 3 retries, verbose false
example.com:8080, 3 retries, verbose false
example.com:443, 3 retries, verbose true
localhost:443, 5 retries, verbose false
```

The rules:

- Defaults must be constants: a number, `true`/`false` or a string literal.
- Once a parameter has a default, every parameter after it needs one too.
- Positional arguments come first, then named ones in any order.
- Naming a parameter that doesn't exist, passing one twice, or leaving out one
  without a default is a compile error.

In Rust this usually needs a builder type or several versions of the function.

## The pipeline operator

From Elixir, F# and Gleam. `x |> f(a)` means `f(x, a)`: the value on the left
becomes the first argument. It lets a chain of steps read in the order they
happen:

```om
fn trim_all(s: str) -> String { s.trim().to_string() }
fn shout(s: String) -> String { s.to_uppercase() }
fn exclaim(s: String, times: usize) -> String { s + "!".repeat(times).as_str() }

fn main() {
    let nested = exclaim(shout(trim_all("  hello  ")), 3)
    let piped = "  hello  " |> trim_all |> shout |> exclaim(3)
    print(nested)
    print(piped)
}
```

```console
$ om run main.om
HELLO!!!
HELLO!!!
```

`|>` binds more loosely than any other operator, so `a + b |> f` means
`f(a + b)`.

## String interpolation

From Python's f-strings, Kotlin and Swift. Any expression can go inside `{}`
in a string literal:

```om
struct Item {
    name: str,
    price: f64,
    qty: i64,
}

fn main() {
    let it = Item { name: "pen", price: 1.25, qty: 4 }
    print("{it.qty} x {it.name} = {it.price * it.qty as f64}")
    print("in stock: {it.qty > 0}, label: {it.name.to_uppercase()}")
    print("use {{ and }} for literal braces")
}
```

```console
$ om run main.om
4 x pen = 5
in stock: true, label: PEN
use { and } for literal braces
```

Rust's `format!` only allows plain variable names inside `{}`. In Om, an
interpolated string builds a `String`. One limit: a string literal can't appear
inside the braces.

## defer

From Zig, Go and Swift. `defer` schedules an expression to run when the
current block ends, whether it ends normally or with `return`. Several defers
run newest first.

```om
fn transfer(amount: i64) -> bool {
    print("lock accounts")
    defer print("unlock accounts")
    if amount <= 0 {
        print("rejected")
        return false
    }
    print("moved {amount}")
    true
}

fn main() {
    print(transfer(50))
    print(transfer(-1))
}
```

```console
$ om run main.om
lock accounts
moved 50
unlock accounts
true
lock accounts
rejected
unlock accounts
false
```

The cleanup sits right next to the setup, so it's hard to forget and easy to
review. The deferred expression sees variables as they are when the block
ends, and `return` isn't allowed inside a `defer`.

`Drop` (see [chapter 4](04-ownership.md)) is still the right tool when the
cleanup belongs to a type, like closing every file. `defer` is for one-off
cleanup in one place.

## Printing anything

One more small difference: `print` shows any value, including your own structs
and enums, without `#[derive(Debug)]` or a format string.

Next: [What's next](11-whats-next.md).
