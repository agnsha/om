# Panics and assert

`panic`, `assert` and `unwrap` print to stderr and exit with code 101.

```om
// assert and panic stop the program with exit code 101, like Rust.
fn checked_div(a: i64, b: i64) -> i64 {
    assert(b != 0, "cannot divide {a} by zero")
    a / b
}

fn main() {
    print(checked_div(10, 2))
    print(checked_div(1, 0))     // panics here
    print("never printed")
}
```

Run it:

```console
$ om run 18-panics.om
5
panic: cannot divide 1 by zero
```

The process exits with code 101.

The source is in [`18-panics.om`](18-panics.om). Back to [Om by Example](README.md).
