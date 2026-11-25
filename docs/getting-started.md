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

## Next

Read the [language reference](language.md) and try the programs in
[`examples/`](../examples).
