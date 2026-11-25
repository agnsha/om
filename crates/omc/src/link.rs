//! Turn the object file Cranelift wrote into an executable.
//!
//! The C runtime (`om_codegen::RUNTIME_C`) is compiled with the object by the
//! system C toolchain:
//!
//! - Linux and macOS: `cc` (or `$CC`).
//! - Windows: the Microsoft C compiler `cl.exe` from Visual Studio or its Build
//!   Tools, found automatically even outside a Developer prompt. If it is not
//!   installed, `gcc` or `clang` on `PATH` (MinGW or LLVM) also work. `$CC`
//!   picks one explicitly.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Link `object` with the runtime into the executable `out`.
pub fn link(object: &[u8], out: &Path) -> Result<(), String> {
    let dir = Scratch::new()?;
    let obj_path = dir.0.join(if cfg!(windows) { "program.obj" } else { "program.o" });
    let rt_path = dir.0.join("om_runtime.c");
    std::fs::write(&obj_path, object).map_err(|e| format!("writing object file: {e}"))?;
    std::fs::write(&rt_path, om_codegen::RUNTIME_C).map_err(|e| format!("writing runtime: {e}"))?;
    // Tools run inside the scratch directory (cl.exe drops files in the current
    // directory), so the output path must not depend on the current directory.
    let out = absolute(out)?;
    let tool = find_tool()?;
    let mut cmd = tool.command();
    cmd.current_dir(&dir.0);
    if tool.msvc {
        cmd.args(["/nologo", "/O2"]).arg(&obj_path).arg(&rt_path);
        let mut fe = OsString::from("/Fe:");
        fe.push(&out);
        cmd.arg(fe).args(["/link", "/INCREMENTAL:NO"]);
    } else {
        cmd.arg(&obj_path).arg(&rt_path).arg("-O2");
        if !cfg!(windows) {
            cmd.arg("-lm");
        }
        cmd.arg("-o").arg(&out);
    }
    let output = cmd.output().map_err(|e| format!("could not run the C compiler `{}`: {e}. {}", tool.name, HELP))?;
    if !output.status.success() {
        let mut msg = format!("linking failed ({}) with `{}`", output.status, tool.name);
        for text in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(text);
            if !text.trim().is_empty() {
                msg.push('\n');
                msg.push_str(text.trim_end());
            }
        }
        return Err(msg);
    }
    Ok(())
}

const HELP: &str = if cfg!(windows) {
    "Install the Visual Studio Build Tools with the \"Desktop development with C++\" workload \
     (see docs/windows.md), or put MinGW gcc or clang on PATH."
} else {
    "Install a C compiler (cc, gcc or clang)."
};

/// A C compiler and how to call it.
struct Tool {
    name: String,
    path: PathBuf,
    /// Extra environment, such as `INCLUDE` and `LIB` for cl.exe.
    env: Vec<(OsString, OsString)>,
    /// Takes cl.exe-style `/flags` rather than gcc-style `-flags`.
    msvc: bool,
}

impl Tool {
    fn plain(name: &str) -> Tool {
        let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name).to_ascii_lowercase();
        Tool { name: name.into(), path: name.into(), env: vec![], msvc: matches!(stem.as_str(), "cl" | "clang-cl") }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.path);
        cmd.envs(self.env.iter().map(|(k, v)| (k, v)));
        cmd
    }
}

fn find_tool() -> Result<Tool, String> {
    if let Some(cc) = std::env::var("CC").ok().filter(|c| !c.trim().is_empty()) {
        return Ok(Tool::plain(&cc));
    }
    if !cfg!(windows) {
        return Ok(Tool::plain("cc"));
    }
    if let Some(tool) = find_msvc() {
        return Ok(tool);
    }
    for name in ["gcc", "clang", "cc"] {
        if on_path(name) {
            return Ok(Tool::plain(name));
        }
    }
    Err(format!("no C compiler found. {HELP}"))
}

#[cfg(windows)]
fn find_msvc() -> Option<Tool> {
    let target = if cfg!(target_arch = "aarch64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" };
    let found = cc::windows_registry::find_tool(target, "cl.exe")?;
    let mut env: Vec<(OsString, OsString)> = found.get_envs().map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
    // cl.exe runs `link.exe` from PATH. Git for Windows ships an unrelated
    // `link.exe` (coreutils) that can come first, so put the MSVC directory,
    // which holds the real linker next to cl.exe, at the front.
    if let Some(dir) = found.path().parent() {
        let current = env
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var_os("PATH"))
            .unwrap_or_default();
        let paths = std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&current));
        if let Ok(joined) = std::env::join_paths(paths) {
            env.retain(|(k, _)| !k.eq_ignore_ascii_case("PATH"));
            env.push(("PATH".into(), joined));
        }
    }
    Some(Tool { name: "cl.exe".into(), path: found.path().to_path_buf(), env, msvc: true })
}

#[cfg(not(windows))]
fn find_msvc() -> Option<Tool> {
    None
}

fn on_path(name: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&paths).any(|dir| {
        let p = dir.join(name);
        p.is_file() || p.with_extension("exe").is_file()
    })
}

fn absolute(p: &Path) -> Result<PathBuf, String> {
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    let cwd = std::env::current_dir().map_err(|e| format!("reading the current directory: {e}"))?;
    Ok(cwd.join(p))
}

/// A private scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("om-{}-{nanos}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("creating temp dir: {e}"))?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
