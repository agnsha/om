//! The three ways to print a diagnostic.
//!
//! * `human`: what people read at a terminal (the default).
//! * `brief`: one line per error plus one per fix, for agents (`--brief` or `OM_AGENT=1`).
//! * `json`: one JSON object per line (`--json`), with the same facts.
//!
//! The brief form costs a fraction of the tokens of the human one: no source
//! excerpt, no decoration, positions as `line:col`.

use crate::{json, line_col, Applicability, Diagnostic, Fix, Severity};

/// Longest source excerpt printed under an error, in characters.
const MAX_EXCERPT: usize = 120;

fn severity_word(d: &Diagnostic) -> &'static str {
    match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// `error[E0200]: message`, location, source line with carets, `help:` lines.
pub fn human(d: &Diagnostic, file: &str, src: &str) -> String {
    let mut out = format!("{}[{}]: {}", severity_word(d), d.code, d.message);
    if let Some(span) = d.span {
        let start = clamp(src, span.start);
        let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[start..].find('\n').map_or(src.len(), |i| start + i);
        let (line_no, col) = line_col(src, start);
        let text = &src[line_start..line_end];
        let width = src[start..clamp(src, span.end).min(line_end).max(start)].chars().count().max(1);
        // Show a window of a long line around the error.
        let skip = (col - 1).saturating_sub(MAX_EXCERPT / 2);
        let shown: String = text.chars().skip(skip).take(MAX_EXCERPT).collect();
        let lead = if skip > 0 { "..." } else { "" };
        let caret_col = col - 1 - skip + lead.len();
        let width = width.min(MAX_EXCERPT.saturating_sub(caret_col).max(1));
        let gutter = line_no.to_string().len();
        let pad = " ".repeat(gutter);
        out.push_str(&format!(
            "\n{pad} --> {file}:{line_no}:{col}\n{pad} |\n{line_no} | {lead}{shown}\n{pad} | {}{}",
            " ".repeat(caret_col),
            "^".repeat(width),
        ));
    } else {
        out.push_str(&format!("\n --> {file}"));
    }
    for f in &d.fixes {
        let (l, c) = line_col(src, f.span.start);
        out.push_str(&format!("\nhelp: {} ({}:{l}:{c}, {})", f.label, file, f.applicability.name()));
    }
    out
}

/// `E0200 file:3:9 message` then `  fix[auto] 3:9 "old"->"new"` per fix.
pub fn brief(d: &Diagnostic, file: &str, src: &str) -> String {
    let mut out = String::new();
    if d.severity == Severity::Warning {
        out.push_str("warning ");
    }
    out.push_str(d.code);
    out.push(' ');
    out.push_str(file);
    if let Some(span) = d.span {
        let (l, c) = line_col(src, span.start);
        out.push_str(&format!(":{l}:{c}"));
    }
    out.push(' ');
    out.push_str(&d.message);
    for f in &d.fixes {
        out.push_str("\n  ");
        out.push_str(&brief_fix(f, src));
    }
    out
}

fn quoted(s: &str) -> String {
    let mut o = String::new();
    json::string(&mut o, s);
    o
}

fn brief_fix(f: &Fix, src: &str) -> String {
    let (l, c) = line_col(src, f.span.start);
    let old = &src[clamp(src, f.span.start)..clamp(src, f.span.end).max(clamp(src, f.span.start))];
    let edit = if old.is_empty() {
        format!("+{}", quoted(&f.text))
    } else if f.text.is_empty() {
        format!("-{}", quoted(old))
    } else {
        format!("{}->{}", quoted(old), quoted(&f.text))
    };
    format!("fix[{}] {l}:{c} {edit}", f.applicability.name())
}

/// One JSON object: code, severity, file, line, col, end_line, end_col, msg, fixes.
/// Each fix replaces `line:col`..`end_line:end_col` with `text`.
pub fn json(d: &Diagnostic, file: &str, src: &str) -> String {
    let mut o = json::Obj::new().str("code", d.code).str("severity", severity_word(d)).str("file", file);
    if let Some(span) = d.span {
        let (l, c) = line_col(src, span.start);
        let (el, ec) = line_col(src, span.end.max(span.start));
        o = o.num("line", l as i64).num("col", c as i64).num("end_line", el as i64).num("end_col", ec as i64);
    }
    o = o.str("msg", &d.message);
    if !d.fixes.is_empty() {
        let fixes = d.fixes.iter().map(|f| {
            let (l, c) = line_col(src, f.span.start);
            let (el, ec) = line_col(src, f.span.end.max(f.span.start));
            json::Obj::new()
                .str("apply", f.applicability.name())
                .num("line", l as i64)
                .num("col", c as i64)
                .num("end_line", el as i64)
                .num("end_col", ec as i64)
                .str("text", &f.text)
                .str("label", &f.label)
                .finish()
        });
        o = o.raw("fixes", &json::array(fixes));
    }
    o.finish()
}

fn clamp(src: &str, pos: usize) -> usize {
    let mut p = pos.min(src.len());
    while !src.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Count fixes `om fix` would apply by default.
pub fn auto_fix_count(diags: &[Diagnostic]) -> usize {
    diags.iter().filter(|d| d.fixes.iter().any(|f| f.applicability == Applicability::Auto)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use om_lexer::Span;

    fn diag() -> (Diagnostic, &'static str) {
        let src = "fn main() {\n    let x = 1\n    x = 2\n}\n";
        let mut d =
            Diagnostic::error("cannot assign twice to `x`; declare it with `let mut x`", Some(Span::new(30, 35)));
        d.fixes.push(Fix {
            applicability: Applicability::Auto,
            span: Span::new(20, 20),
            text: "mut ".into(),
            label: "declare `x` with `let mut`".into(),
        });
        (d, src)
    }

    #[test]
    fn brief_is_one_line_per_error_and_fix() {
        let (d, src) = diag();
        assert_eq!(
            brief(&d, "a.om", src),
            "E0600 a.om:3:5 cannot assign twice to `x`; declare it with `let mut x`\n  fix[auto] 2:9 +\"mut \""
        );
    }

    #[test]
    fn json_has_positions_and_fixes() {
        let (d, src) = diag();
        let j = json(&d, "a.om", src);
        assert!(
            j.starts_with(
                r#"{"code":"E0600","severity":"error","file":"a.om","line":3,"col":5,"end_line":3,"end_col":10,"#
            ),
            "{j}"
        );
        assert!(
            j.contains(r#""fixes":[{"apply":"auto","line":2,"col":9,"end_line":2,"end_col":9,"text":"mut ""#),
            "{j}"
        );
    }

    #[test]
    fn human_shows_long_lines_as_a_window() {
        let src = format!("fn main() {{ let x = {}y }}", "1 + ".repeat(200));
        let pos = src.find('y').unwrap();
        let d = Diagnostic::error("cannot find variable `y`", Some(Span::new(pos, pos + 1)));
        let out = human(&d, "a.om", &src);
        assert!(out.lines().all(|l| l.chars().count() < 140), "{out}");
        let lines: Vec<&str> = out.lines().collect();
        let code_line = lines[3];
        let caret_line = lines[4];
        let caret = caret_line.find('^').unwrap();
        assert_eq!(&code_line[caret..caret + 1], "y", "{out}");
    }
}
