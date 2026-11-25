//! The Om compiler driver: parse, check, generate an object file, and link.

use om_lexer::Span;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A compiler error with its location, ready to print.
#[derive(Debug)]
pub struct Diagnostic {
    pub message: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    fn plain(message: impl Into<String>) -> Self {
        Diagnostic { message: message.into(), span: None }
    }

    /// Render as `file:line:col: error: message` followed by the source line and a caret.
    pub fn render(&self, file: &str, src: &str) -> String {
        let Some(span) = self.span.filter(|s| s.end > 0 || s.start > 0) else {
            return format!("error: {}", self.message);
        };
        let start = span.start.min(src.len());
        let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[start..].find('\n').map_or(src.len(), |i| start + i);
        let line_no = src[..start].matches('\n').count() + 1;
        let col = src[line_start..start].chars().count() + 1;
        let text = &src[line_start..line_end];
        let width = src[start..span.end.min(line_end).max(start)].chars().count().max(1);
        let gutter = line_no.to_string().len();
        format!(
            "error: {msg}\n{pad} --> {file}:{line_no}:{col}\n{pad} |\n{line_no} | {text}\n{pad} | {sp}{carets}",
            msg = self.message,
            pad = " ".repeat(gutter),
            sp = " ".repeat(col - 1),
            carets = "^".repeat(width),
        )
    }
}

/// Parse and type-check `src`, returning the object file bytes.
pub fn compile(src: &str) -> Result<Vec<u8>, Diagnostic> {
    let program = om_parser::parse(src).map_err(|e| Diagnostic { message: e.message, span: Some(e.span) })?;
    om_codegen::compile_to_object(&program).map_err(|e| Diagnostic { message: e.message, span: Some(e.span) })
}

/// Compile `src` and link it into a native executable at `out`.
pub fn build(src: &str, out: &Path) -> Result<(), Diagnostic> {
    let object = compile(src)?;
    let dir = scratch_dir()?;
    let obj_path = dir.join("program.o");
    let rt_path = dir.join("om_runtime.c");
    std::fs::write(&obj_path, object).map_err(|e| Diagnostic::plain(format!("writing object file: {e}")))?;
    std::fs::write(&rt_path, om_codegen::RUNTIME_C).map_err(|e| Diagnostic::plain(format!("writing runtime: {e}")))?;
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let status =
        Command::new(&cc).arg(&obj_path).arg(&rt_path).arg("-O2").arg("-o").arg(out).status().map_err(|e| {
            Diagnostic::plain(format!("could not run the linker `{cc}`: {e}. Install a C compiler (cc, gcc or clang)."))
        })?;
    let _ = std::fs::remove_dir_all(&dir);
    if !status.success() {
        return Err(Diagnostic::plain(format!("linking failed ({status})")));
    }
    Ok(())
}

fn scratch_dir() -> Result<PathBuf, Diagnostic> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("om-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| Diagnostic::plain(format!("creating temp dir: {e}")))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_caret_under_error() {
        let src = "fn main() {\n    let x = y\n}";
        let d = compile(src).unwrap_err();
        let out = d.render("t.om", src);
        assert!(out.contains("cannot find variable `y`"), "{out}");
        assert!(out.contains("t.om:2:13"), "{out}");
        assert!(out.ends_with("            ^"), "{out}");
    }
}
