# 1. Getting going

## Install

Follow [Install Om](../install.md). When `om --version` prints a version,
you're ready.

## Hello, world

Create `main.om`:

```om
fn main() {
    print("Hello, world!")
}
```

```console
$ om run main.om
Hello, world!
```

Here's what's going on:

- `fn main()` declares the function every program starts in. It takes no
  parameters.
- The body is between `{` and `}`.
- `print` is built in. It prints any value followed by a newline.
- There's no semicolon. In Om they're optional; use them only if you want to
  put two statements on one line.

## Compiling and running are two steps

`om run` does two things at once. You can do them separately:

```console
$ om build main.om -o hello
$ ./hello
Hello, world!
```

`om build` turns your source into native machine code using Cranelift, then
asks your system's C compiler to link it into an executable. The executable
doesn't need Om to run. This is the same model as Rust, C and Go, and the
opposite of Python or JavaScript, where an interpreter reads your code each
time.

`om check main.om` does only the first half: it reads and type-checks the
program without producing code. It's the fastest way to ask "is this right?"

## How an Om program is laid out

An Om file is a list of items: functions (`fn`), types (`struct`, `enum`),
`impl` blocks, `trait`s and `const`s. They can appear in any order, and a
function can call one defined further down:

```om
fn main() {
    greet("Om")
}

fn greet(name: str) {
    print("Hello, {name}!")
}
```

```console
$ om run main.om
Hello, Om!
```

`{name}` inside a string inserts the value of `name`. You can put any
expression there, such as `{2 + 2}`.

There's no module system yet, so a program is one file. Modules are next on
the roadmap.

Next: [A first project](02-grade-report.md).
