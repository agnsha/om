//! The Om compiler driver: parse, check, generate an object file, and link.

pub mod link;

use om_lexer::Span;
use std::path::Path;

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
    link::link(&object, out).map_err(Diagnostic::plain)
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
