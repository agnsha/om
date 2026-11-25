# Hello, world

The smallest Om program. `print` shows any value: numbers, strings, tuples, structs and enums, with no `derive` needed.

```om
// Every program starts at `fn main`. `print` writes any value and a newline.
fn main() {
    print("Hello, Om!")
    print(42)
    print(true)
    print((1, "two", 3.5))
}
```

Run it:

```console
$ om run 01-hello.om
Hello, Om!
42
true
(1, "two", 3.5)
```

The source is in [`01-hello.om`](01-hello.om). Back to [Om by Example](README.md).
