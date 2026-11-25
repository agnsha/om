# Om on Windows

Om runs natively on Windows 10 and 11 (64-bit). Everything below happens in
**PowerShell**. You do not need WSL.

Om turns your program into a real `.exe`. To do that it uses Microsoft's C
compiler and linker, so every way of installing Om starts with step 1.

## 1. Install the Visual Studio Build Tools (once)

Open PowerShell and run:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

A Visual Studio installer window appears and closes when it is done (about
5 to 15 minutes, roughly 5 GB). If you already have Visual Studio with
**Desktop development with C++**, skip this step.

> No `winget`? Download the Build Tools from
> <https://visualstudio.microsoft.com/visual-cpp-build-tools/>, run it, tick
> **Desktop development with C++**, and click Install.

## 2a. Easiest: download `om.exe`

1. Open the repository's **Releases** page on GitHub and download
   `om-x86_64-pc-windows-msvc.zip` from the latest release.
2. Make a folder for it and unzip it there, for example `C:\om\bin`.
3. Add that folder to your PATH so `om` works everywhere:

   ```powershell
   [Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\om\bin", "User")
   ```

4. **Close PowerShell and open a new one**, then check it works:

   ```powershell
   om --version
   ```

## 2b. Or build `om` from source

This needs Rust and Git as well:

```powershell
winget install Rustlang.Rustup
winget install Git.Git
```

**Close PowerShell and open a new one** (so it finds `cargo` and `git`), then:

```powershell
cd $HOME
git clone https://github.com/agnsha/om
cd om
cargo install --path crates/omc
om --version
```

`git clone` opens a browser window to sign in to GitHub the first time,
because the repository is private. `cargo install` must run inside the `om`
folder: it builds `crates/omc` from there.

## 3. Run something

From the `om` folder:

```powershell
om run examples/hello.om
om run games/hollow-bell/hollow_bell.om
om run games/emberdeep/emberdeep.om
```

The games need a terminal with colour; Emberdeep's 3D view needs true colour,
which Windows Terminal (the default on Windows 11) has.

Build a standalone program you can share. The `.exe` needs nothing else
installed to run:

```powershell
om build examples/fib.om -o fib.exe
.\fib.exe
```

Colours, Unicode text and arrow keys work in Windows Terminal, PowerShell and
the classic console. Om switches the console to UTF-8 and turns on colour
codes while your program runs, and puts it back afterwards.

## Troubleshooting

| You see | What to do |
| --- | --- |
| `cargo : The term 'cargo' is not recognized` | Rust is not installed, or PowerShell was open while you installed it. Do step 2b, then open a new PowerShell window. |
| `om : The term 'om' is not recognized` | `om` is not installed or not on PATH. Do step 2a or 2b, then open a new PowerShell window. |
| `error: no C compiler found` or `could not run the C compiler` | The Build Tools are missing. Do step 1. |
| `linker 'link.exe' not found` while running `cargo install` | Same as above: Rust needs the Build Tools too. Do step 1. |
| `link: extra operand ...` and `Try 'link --help'` while running `cargo install` | Rust could not find the Build Tools, so it ran Git's unrelated `link` command instead. Do step 1, open a new PowerShell, and run `cargo install` again. If it still fails, see [Git's `link.exe`](#gits-linkexe) below. |
| `error: could not find Cargo.toml` or `--path must point to a directory` | You are not in the `om` folder. `cd` into the folder you cloned (for example `cd C:\om-src`) and run the command again. |
| Strange symbols like `←[31m` instead of colours | You are on an old Windows 10 build. Use [Windows Terminal](https://aka.ms/terminal). |

## Other C compilers

Om looks for Microsoft's `cl.exe` first, even outside a "Developer PowerShell".
If it is not installed, Om uses `gcc` or `clang` from your PATH (for example
from MinGW-w64 or LLVM). To pick one yourself, set `CC`:

```powershell
$env:CC = "clang"
om run examples/hello.om
```

## Git's `link.exe`

Git for Windows comes with Linux tools, and one of them is also called
`link.exe`. When the Build Tools are missing, Rust falls back to whatever
`link.exe` is on your PATH, finds Git's, and fails with `link: extra operand`.

1. Install the Build Tools (step 1). This is almost always the whole fix.
2. If it still fails, run the install from a **Developer PowerShell**, which
   puts Microsoft's linker first on PATH. Open the Start menu, search for
   **Developer PowerShell for VS 2022**, open it, then:

   ```powershell
   cd C:\om-src     # the folder you cloned Om into
   cargo install --path crates/omc
   ```

After `om` is installed, any PowerShell window works. `om` finds Microsoft's
compiler and linker by itself and never uses Git's `link.exe`.
