# Getting started

This page takes about ten minutes. You'll write a program, run it, build it into
an executable, and read your first compiler error. You need Om installed
first: see [Install Om](install.md).

## 1. Hello, Om

Make a folder and a file called `hello.om`:

```om
fn main() {
    print("Hello, Om!")
}
```

Run it:

```console
$ om run hello.om
Hello, Om!
```

`om run` compiles the file to native machine code, runs it, and deletes the
executable afterwards. There is no interpreter or virtual machine in between.

## 2. Build an executable

```console
$ om build hello.om -o hello
$ ./hello
Hello, Om!
```

Leave out `-o` and the executable is named after the file (`hello`). The
result is a normal program: copy it anywhere on the same kind of machine and
it runs without Om installed.

## 3. Something a bit bigger

Save this as `fizzbuzz.om`:

```om
fn fizzbuzz(n: i64) -> String {
    match (n % 3, n % 5) {
        (0, 0) => "FizzBuzz".to_string(),
        (0, _) => "Fizz".to_string(),
        (_, 0) => "Buzz".to_string(),
        _ => n.to_string(),
    }
}

fn main() {
    for i in 1..=15 {
        print(fizzbuzz(i))
    }
}
```

```console
$ om run fizzbuzz.om
1
2
Fizz
4
Buzz
Fizz
7
8
Fizz
Buzz
11
Fizz
13
14
FizzBuzz
```

A few things to notice if you know Rust:

- Semicolons are optional.
- `print` takes any value. There's no `println!` macro and no format string.
- `match` works on a tuple, and the last expression in a block is its value.

## 4. Read an error

Change `fizzbuzz` so it forgets a case:

```om
fn fizzbuzz(n: i64) -> String {
    match (n % 3, n % 5) {
        (0, 0) => "FizzBuzz".to_string(),
        (0, _) => "Fizz".to_string(),
    }
}
```

```console
$ om check fizzbuzz.om
error: `match` does not cover every case; missing: (i64::MIN..=-1, _) (add an arm for it or a `_` arm)
  --> fizzbuzz.om:2:5
  |
2 |     match (n % 3, n % 5) {
  |     ^^^^^^^^^^^^^^^^^^^^^^
```

The compiler even found a case you might not think of: `n % 3` is negative when
`n` is negative. Every error names the file, line and column, shows the line,
and usually says how to fix it. `om check` stops after type checking, which makes it the quickest
way to see whether a file is correct. The [compiler errors](errors.md) page
lists the common ones.

## 5. Exit codes

`main` can return an `i64`, which becomes the process exit code:

```om
fn main() -> i64 {
    print("something went wrong")
    1
}
```

A panic (`panic(...)`, a failed `assert`, `unwrap` on `None`) prints a message
to stderr and exits with code 101, the same as Rust.

## Next steps

- New to this kind of language? Read [the Om Book](book/README.md).
- Prefer code to prose? Go through [Om by Example](by-example/README.md).
- Coming from Rust? [Om for Rust programmers](om-for-rust.md) is one page.
- Try the programs in the repository's `examples/` folder.
