//! The Om compiler driver: parse, check, generate an object file, and link,
//! plus the agent-facing tools built on top (`om check --json`, `om fix`,
//! `om doc`, `om guide`, `om test`, `om fmt`, `om audit`).
//!
//! Every error carries a stable code (see [`codes`]) and, where the compiler
//! can tell, machine-applicable fixes. One run reports every error it can.

pub mod audit;
pub mod codes;
pub mod doc;
pub mod fix;
pub mod fmt;
pub mod guide;
pub mod json;
pub mod lints;
pub mod render;
pub mod sha256;
pub mod suggest;
pub mod testing;
pub mod tokens;

use om_lexer::Span;
use om_parser::ast::Program;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How sure a fix is. Mirrors rustc's applicability levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    /// Safe to apply without looking: `om fix` applies these.
    Auto,
    /// Probably right, but check the result (`om fix --maybe` applies these).
    Maybe,
    /// Shows the shape of the fix, with `TODO` where you must fill something in.
    Placeholder,
}

impl Applicability {
    pub fn name(self) -> &'static str {
        match self {
            Applicability::Auto => "auto",
            Applicability::Maybe => "maybe",
            Applicability::Placeholder => "placeholder",
        }
    }
}

/// Replace the source text in `span` with `text` (an insertion when the span is empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub applicability: Applicability,
    pub span: Span,
    pub text: String,
    /// A few words for people: "did you mean `count`?".
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A compiler error or warning with its location, code and fixes.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Stable code, such as `E0200`; `om explain CODE` describes it.
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub fixes: Vec<Fix>,
}

impl Diagnostic {
    /// A diagnostic for a compiler message, with its code looked up.
    pub fn error(message: impl Into<String>, span: Option<Span>) -> Self {
        let message = message.into();
        let code = codes::classify(&message).0.code;
        let span = span.filter(|s| s.start > 0 || s.end > 0);
        Diagnostic { code, severity: Severity::Error, message, span, fixes: vec![] }
    }

    fn plain(message: impl Into<String>) -> Self {
        Diagnostic::error(message, None)
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Human-readable form: `error[E0200]: message`, the location, the source
    /// line with a caret, and one `help:` line per fix.
    pub fn render(&self, file: &str, src: &str) -> String {
        render::human(self, file, src)
    }
}

/// Largest source file the compiler reads (16 MiB).
pub const MAX_SOURCE_BYTES: u64 = 16 << 20;

/// Read a source file, refusing huge files, devices and invalid UTF-8.
pub fn read_source(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    let meta = file.metadata().map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("`{}` is not a regular file", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(format!("`{}` is larger than {} MiB", path.display(), MAX_SOURCE_BYTES >> 20));
    }
    String::from_utf8(bytes).map_err(|_| format!("`{}` is not valid UTF-8", path.display()))
}

/// Run `f` on a thread with a large stack. The compiler recurses over the
/// syntax tree; the parser caps nesting, and this gives every pass headroom.
fn with_big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("om-compiler".into())
            .stack_size(256 << 20)
            .spawn_scoped(s, f)
            .expect("spawn compiler thread")
            .join()
            .unwrap_or_else(|p| std::panic::resume_unwind(p))
    })
}

/// Everything one compiler run found.
pub struct Analysis {
    /// Errors first, then warnings, each sorted by position.
    pub diagnostics: Vec<Diagnostic>,
    /// The object file, when asked for and there were no errors.
    pub object: Option<Vec<u8>>,
    /// The parsed program (partial if there were syntax errors).
    pub program: Program,
}

/// Parse, lint and type-check `src`, collecting every error and warning with
/// codes and fixes. With `emit`, also generate machine code.
pub fn analyze(src: &str, emit: bool) -> Analysis {
    with_big_stack(|| analyze_inner(src, emit))
}

fn analyze_inner(src: &str, emit: bool) -> Analysis {
    let (program, parse_errors) = om_parser::parse_all(src);
    let mut diags: Vec<Diagnostic> =
        parse_errors.into_iter().map(|e| Diagnostic::error(e.message, Some(e.span))).collect();
    let mut object = None;
    if diags.is_empty() {
        diags.extend(lints::check(&program));
        match om_codegen::compile_all(&program, false, emit) {
            Ok(out) => {
                for (name, span) in out.unchecked_generics {
                    let mut d = Diagnostic::error(
                        format!("generic function `{name}` is never called, so its body was not type-checked"),
                        Some(span),
                    );
                    d.severity = Severity::Warning;
                    diags.push(d);
                }
                if !diags.iter().any(Diagnostic::is_error) {
                    object = out.object;
                }
            }
            Err(errors) => {
                diags.extend(errors.into_iter().map(|e| Diagnostic::error(e.message, Some(e.span))));
            }
        }
    }
    // Generic instances can report the same error once per instantiation.
    let mut seen = std::collections::HashSet::new();
    diags.retain(|d| seen.insert((d.message.clone(), d.span.map(|s| (s.start, s.end)))));
    diags.sort_by_key(|d| (!d.is_error(), d.span.map_or(0, |s| s.start)));
    let ctx = suggest::Context::new(src, &program);
    for d in &mut diags {
        suggest::enrich(d, &ctx);
    }
    Analysis { diagnostics: diags, object, program }
}

/// Check `src` without generating machine code: every error and warning.
pub fn check(src: &str) -> Vec<Diagnostic> {
    analyze(src, false).diagnostics
}

/// Parse and type-check `src`, returning the object file bytes, or the first error.
pub fn compile(src: &str) -> Result<Vec<u8>, Diagnostic> {
    let mut a = analyze(src, true);
    match a.object.take() {
        Some(obj) => Ok(obj),
        None => Err(a
            .diagnostics
            .into_iter()
            .find(Diagnostic::is_error)
            .unwrap_or_else(|| Diagnostic::plain("internal error: compilation failed without a diagnostic"))),
    }
}

/// Compile `src` and link it into a native executable at `out`.
pub fn build(src: &str, out: &Path) -> Result<(), Diagnostic> {
    let object = compile(src)?;
    link(&object, out)
}

/// Link an object file with the Om runtime into an executable at `out`.
pub fn link(object: &[u8], out: &Path) -> Result<(), Diagnostic> {
    let dir = TempDir::new().map_err(|e| Diagnostic::plain(format!("creating temp dir: {e}")))?;
    let obj_path = dir.path().join("program.o");
    let rt_path = dir.path().join("om_runtime.c");
    write_new(&obj_path, object).map_err(|e| Diagnostic::plain(format!("writing object file: {e}")))?;
    write_new(&rt_path, om_codegen::RUNTIME_C.as_bytes())
        .map_err(|e| Diagnostic::plain(format!("writing runtime: {e}")))?;
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let status = Command::new(&cc)
        .arg(&obj_path)
        .arg(&rt_path)
        .arg("-O2")
        .arg("-lm")
        .arg("-o")
        .arg(safe_arg_path(out))
        .status()
        .map_err(|e| {
            Diagnostic::plain(format!("could not run the linker `{cc}`: {e}. Install a C compiler (cc, gcc or clang)."))
        })?;
    if !status.success() {
        return Err(Diagnostic::plain(format!("linking failed ({status})")));
    }
    Ok(())
}

/// Start `cmd`. Linux can briefly refuse to run a just-linked program with
/// "text file busy" while another thread forks; that is retried.
pub fn spawn(cmd: &mut Command) -> std::io::Result<std::process::Child> {
    let mut tries = 0;
    loop {
        match cmd.spawn() {
            Err(e) if e.raw_os_error() == Some(26) && tries < 50 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            r => return r,
        }
    }
}

/// A relative path that starts with `-` would be read by `cc` as an option.
fn safe_arg_path(p: &Path) -> PathBuf {
    if p.is_relative() && p.to_string_lossy().starts_with('-') {
        Path::new(".").join(p)
    } else {
        p.to_path_buf()
    }
}

/// Create `path` (which must not exist yet) and write `bytes` to it.
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)
}

/// A private scratch directory, removed on drop.
///
/// It is created fresh with a random name and mode 0700, and creation fails
/// rather than reusing an existing path. A predictable name in a shared `/tmp`
/// would let another local user plant a symlink or swap the runtime source or
/// the built program between writing and running it.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> std::io::Result<TempDir> {
        let base = std::env::temp_dir();
        let mut last = None;
        for _ in 0..16 {
            let dir = base.join(format!("om-{}-{:016x}", std::process::id(), random_u64()));
            let mut b = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
            match b.create(&dir) {
                Ok(()) => return Ok(TempDir(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
                Err(e) => return Err(e),
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::other("could not create a temp dir")))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 64 random bits from the OS-seeded hasher keys in the standard library.
pub fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.finish()
}

/// Line (1-based) and column (1-based, in characters) of byte offset `pos`.
pub fn line_col(src: &str, pos: usize) -> (usize, usize) {
    let mut pos = pos.min(src.len());
    while !src.is_char_boundary(pos) {
        pos -= 1;
    }
    let line_start = src[..pos].rfind('\n').map_or(0, |i| i + 1);
    let line = src[..pos].matches('\n').count() + 1;
    (line, src[line_start..pos].chars().count() + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_code_and_caret_under_error() {
        let src = "fn main() {\n    let x = y\n}";
        let d = compile(src).unwrap_err();
        assert_eq!(d.code, "E0200");
        let out = d.render("t.om", src);
        assert!(out.starts_with("error[E0200]: cannot find variable `y`"), "{out}");
        assert!(out.contains("t.om:2:13"), "{out}");
        assert!(out.contains("            ^"), "{out}");
    }

    #[test]
    fn reports_every_error_in_one_run() {
        let src = "fn a() -> i64 { x }\nfn b() { let s: str = 1 }\nfn main() { c() }\n";
        let d = check(src);
        let codes: Vec<&str> = d.iter().map(|d| d.code).collect();
        assert_eq!(codes, ["E0200", "E0301", "E0201"], "{d:?}");
    }

    #[test]
    fn reports_every_syntax_error_in_one_run() {
        let src = "fn a() {\n    let x = )\n    let y = 2\n    let z = ]\n}\nfn main() { @ }\n";
        let d = check(src);
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(d.iter().all(|d| d.code == "E0100" || d.code == "E0103"), "{d:?}");
    }

    #[test]
    fn temp_dirs_are_private_and_unique() {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();
        assert_ne!(a.path(), b.path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(a.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        let p = a.path().to_path_buf();
        drop(a);
        assert!(!p.exists());
    }

    #[test]
    fn output_paths_cannot_become_linker_options() {
        assert_eq!(safe_arg_path(Path::new("-lfoo")), Path::new("./-lfoo"));
        assert_eq!(safe_arg_path(Path::new("out")), Path::new("out"));
    }

    #[test]
    fn refuses_non_files_and_huge_files() {
        let dir = TempDir::new().unwrap();
        assert!(read_source(dir.path()).unwrap_err().contains("not a regular file"));
        let p = dir.path().join("x.om");
        std::fs::write(&p, [0xff, 0xfe]).unwrap();
        assert!(read_source(&p).unwrap_err().contains("UTF-8"));
    }
}
