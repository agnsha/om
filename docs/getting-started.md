# Getting started

## Requirements

- Rust stable (install with [rustup](https://rustup.rs))
- A C compiler for linking: `cc`, `gcc` or `clang`, or on Windows the Visual
  Studio Build Tools (found automatically). Set `CC` to choose one.
- Linux or macOS (x86-64 or ARM64), or Windows (x86-64). Windows users: see
  [the Windows guide](windows.md) for step-by-step setup in PowerShell.

## Install

```sh
git clone https://github.com/agnsha/om
cd om
cargo install --path crates/omc
om --version
```

Without installing, use `cargo run --bin om -- <args>` from the repository.

## Your first program

Create `hello.om`:

```om
fn main() {
    print("Hello, Om!")
}
```

Run it:

```sh
om run hello.om
```

Or build a standalone executable:

```sh
om build hello.om -o hello
./hello
```

## Exit codes

If `main` returns an `i64`, it becomes the process exit code:

```om
fn main() -> i64 {
    print("something went wrong")
    1
}
```

## Check, fix and test

```sh
om check app.om     # every error at once, each with a code: om explain E0200
om fix app.om       # apply the safe fixes
om test app.om      # run every fn test_*() in the file
om fmt app.om       # canonical layout
```

A test is a function whose name starts with `test_`:

```om
fn double(x: i64) -> i64 { x * 2 }

fn test_double() {
    assert_eq(double(2), 4)
}

fn main() { print(double(21)) }
```

If you use an AI coding agent, have it run `om guide` once per session and set
`OM_AGENT=1` for compact errors.

## Next

Read the [language reference](language.md) and try the programs in
[`examples/`](../examples).
