# Tools

Everything in Om goes through one command, `om`. This page lists what it does
today, what is written but still in review, and what is planned.

## Available now

| Command | What it does |
|---|---|
| `om run <file.om>` | Compile to native code, run it, then delete the executable. The program's exit code becomes `om`'s exit code. |
| `om build <file.om> [-o <out>]` | Compile to a standalone executable. Without `-o` it is named after the file. |
| `om check <file.om>` | Parse and type-check only, without generating code. Prints `ok: <file>` or the first error. The fastest way to check your work. |
| `om --version` | Print the version. |
| `om help` | Print the usage summary. |

```console
$ om check app.om
ok: app.om
$ om build app.om -o app
$ ./app
```

Exit codes: `0` on success, `1` for a compile error, `2` for a usage mistake
(such as a missing file name), and whatever your program returns under
`om run`.

### Environment variables

| Variable | Effect |
|---|---|
| `CC` | The C compiler used to link, `cc` by default. For example `CC=clang om build app.om`. |

## In review

These commands are written and tested but waiting to be merged into the main
branch ([pull request #2](https://github.com/agnsha/om/pull/2)). They are
built to make Om cheap for AI coding assistants to work with: short error
output, machine-readable JSON, and fixes the tool can apply by itself.

| Command | What it does |
|---|---|
| `om check --brief` / `--json` | Every error at once, each with a stable error code and a suggested fix. |
| `om fix <file.om>` | Apply the suggested fixes automatically until the file compiles. `--dry-run` shows them first. |
| `om explain <CODE>` | The long explanation of an error code. |
| `om fmt <file.om>` | Rewrite a file in the standard layout. `--check` only reports. |
| `om test <file.om>` | Run every `fn test_*()` in the file and print only the failures. |
| `om doc [NAME]` | Show signatures and docs for the standard library and your file. |
| `om guide` | Print the whole language on one compact card, written for AI assistants. |
| `om audit <file.om>` | List which functions do I/O or can panic. |
| `om watch <file.om>` | Re-check the file every time you save. |
| `om tokens <file>` | Estimate how many LLM tokens a file costs. |

Until that lands, `om fmt`, `om test` and the others answer with the usage
message.

## Planned

| Tool | Notes |
|---|---|
| Package manager and project files | Like Cargo: a project manifest, dependencies, `om new`. Comes with modules (`mod`, `use`, `pub`). |
| Language server | Completion, go-to-definition and inline errors in editors. |
| Editor extensions | Syntax highlighting for VS Code and others. Use Rust highlighting until then. |
| Release builds through LLVM | Today every build uses Cranelift, which compiles fast. An optimising LLVM backend for release builds is phase 5 of the roadmap. |
| Benchmarks | A published suite comparing Om with Rust, C and Go, losses included. |
| [Playground](playground.md) | Running Om in the browser. |

## Working with the compiler source

If you're hacking on Om itself:

```sh
cargo test --workspace          # unit tests and every example's expected output
cargo run --bin om -- run x.om  # use your changed compiler without installing it
```

The repository's `CONTRIBUTING.md` and `docs/architecture.md` explain how the
compiler is put together.
