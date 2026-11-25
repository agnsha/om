# Playground

**Status: planned.** There is no web playground for Om yet. This page explains
what it will be and how to get the same quick edit-and-run loop on your own
machine today.

## What the playground will be

A page where you type Om code, press Run, and see the output, without
installing anything. Like the Rust playground, it should be able to:

- run a program and show its output and exit code,
- show compiler errors with the same arrows and hints as the command line,
- share a program as a link,
- load any program from [Om by Example](by-example/README.md) with one click.

Om compiles to native machine code, so the playground needs a small server that
compiles and runs code in a sandbox, or a build of the compiler for
WebAssembly. Neither exists yet.

## The playground on your machine, today

Three commands get you close:

```sh
mkdir scratch && cd scratch
printf 'fn main() {\n    print("Hello, Om!")\n}\n' > play.om
om run play.om
```

Then keep the file open in your editor and re-run after each change. Some
habits that help:

- `om check play.om` is faster than `om run` when you only want to know if the
  code is right.
- On Linux and macOS, re-run on every save with any file watcher, for example
  `ls play.om | entr -c om run play.om` (install `entr` first).
- Copy any program from [Om by Example](by-example/README.md) into `play.om`
  and change it to see what happens.

An `om watch` command that does the re-run for you is written and in review;
see [Tools](tools.md#in-review).
