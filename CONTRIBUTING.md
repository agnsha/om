# Contributing to Om

Thanks for your interest! Om is young, so small contributions make a big
difference: a clearer error message, a new example, a test for an edge case.

## Setup

```sh
git clone https://github.com/agnsha/om
cd om
cargo test
cargo run --bin om -- run examples/hello.om
```

You need Rust stable and a C compiler (`cc`, `gcc` or `clang`).

## Before opening a pull request

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all
```

CI runs the same checks on Linux, macOS and Windows. The pull request
template has a short checklist; fill it in so reviewers know how you tested.

## Automated review

Pull requests are reviewed automatically by [CodeRabbit](https://coderabbit.ai),
configured in [.coderabbit.yaml](.coderabbit.yaml). Its comments are
suggestions: reply to them in the PR (mention `@coderabbitai`) if you disagree
or want it to look again. A maintainer still approves and merges.

## Where to start

- The [website](https://om-lang.pages.dev) and `om guide` to learn the language first

- Issues labelled **good first issue**
- [docs/architecture.md](docs/architecture.md) has a step-by-step guide for adding a language feature
- New example programs: add `examples/name.om` and `examples/name.expected`; the test suite runs them automatically

## Language changes

Changes to syntax or semantics start as an issue using the *Language proposal*
template, so the design can be discussed before code is written.

## Code of conduct

Be kind and assume good intent. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
