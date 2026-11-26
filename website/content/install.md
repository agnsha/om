# Install Om

Om is built from source for now. There are no prebuilt downloads or installer
script yet (both are **planned**). Building takes a couple of minutes and needs
two things you may already have.

## What you need

| Need | Why | Get it |
|---|---|---|
| Rust, stable | The Om compiler is written in Rust | [rustup.rs](https://rustup.rs) |
| A C compiler: `cc`, `gcc` or `clang` | Om uses it to link your program into an executable | See below |
| Git | To download the source | [git-scm.com](https://git-scm.com) |

Supported systems: **Linux** and **macOS**, on x86-64 or ARM64, plus Windows
through WSL. Those are the systems the test suite runs on.

> The Om repository is private for now, so `git clone` needs a GitHub account
> that has been given access.

## Linux

```sh
# Debian / Ubuntu
sudo apt install build-essential git
# Fedora
sudo dnf install gcc git

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/agnsha/om
cd om
cargo install --path crates/omc
om --version
```

## macOS

```sh
xcode-select --install          # gives you clang and git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/agnsha/om
cd om
cargo install --path crates/omc
om --version
```

## Windows

On the main branch today, Om runs on Windows through **WSL** (Windows
Subsystem for Linux), which runs Om exactly like Linux:

1. Open PowerShell as administrator and run `wsl --install`, then restart.
2. Open the **Ubuntu** app from the Start menu.
3. Follow the [Linux](#linux) steps above inside it.

Your Windows drives are under `/mnt/` in WSL, so a project in `C:\om` is at
`/mnt/c/om`.

> **In review: native Windows.** Running Om straight from PowerShell, with no
> WSL, is written and tested in [pull request #4](https://github.com/agnsha/om/pull/4).
> The steps are on the [Om on Windows](windows.md) page and work once that
> change is merged.

## Check it works

```sh
cd om
om run examples/hello.om
```

You should see:

```text
Hello, Om!
```

## Without installing

From inside the `om` folder you can run the compiler through Cargo:

```sh
cargo run --release --bin om -- run examples/hello.om
```

## Choosing the C compiler

Om calls `cc` by default. Set `CC` to use another one:

```sh
CC=clang om build app.om
```

If no C compiler is found, Om says so:

```text
error: could not run the linker `cc`: ... Install a C compiler (cc, gcc or clang).
```

## Updating

```sh
cd om
git pull
cargo install --path crates/omc --force
```

## Uninstalling

```sh
cargo uninstall omc
```

## Editor support

There is no editor extension or language server yet (**planned**). Until then,
Rust syntax highlighting is a close fit for `.om` files: in VS Code, open an
`.om` file, click the language name in the status bar and pick Rust.

Next: [Getting started](getting-started.md).
