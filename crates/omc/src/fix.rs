//! `om fix`: apply machine-applicable fixes until none are left.

use crate::{Applicability, Diagnostic, Fix};

/// Most check-and-fix rounds. Each round can unblock the next error (a parse
/// error hides type errors), so a few rounds settle most files.
pub const MAX_ROUNDS: usize = 8;

/// One applied fix, for reporting.
#[derive(Debug, Clone)]
pub struct Applied {
    pub code: &'static str,
    pub label: String,
}

/// Apply `fixes` to `src`. Overlapping fixes are skipped (the first one wins),
/// and so are fixes whose span is out of range or splits a character.
pub fn apply(src: &str, fixes: &[&Fix]) -> (String, usize) {
    let mut sorted: Vec<&Fix> = fixes.to_vec();
    sorted.sort_by_key(|f| (f.span.start, f.span.end));
    let mut chosen: Vec<&Fix> = Vec::new();
    for f in sorted {
        let ok = f.span.start <= f.span.end
            && f.span.end <= src.len()
            && src.is_char_boundary(f.span.start)
            && src.is_char_boundary(f.span.end);
        let overlaps = chosen
            .last()
            .is_some_and(|p| f.span.start < p.span.end || (f.span.start == p.span.start && f.span.start == f.span.end));
        if ok && !overlaps {
            chosen.push(f);
        }
    }
    let mut out = src.to_string();
    for f in chosen.iter().rev() {
        out.replace_range(f.span.start..f.span.end, &f.text);
    }
    (out, chosen.len())
}

/// The fixes `om fix` applies for these diagnostics: each diagnostic's fixes of
/// the allowed applicability (they are written to apply together).
pub fn selected(diags: &[Diagnostic], maybe: bool) -> Vec<&Fix> {
    diags
        .iter()
        .flat_map(|d| {
            let ok =
                |f: &&Fix| f.applicability == Applicability::Auto || (maybe && f.applicability == Applicability::Maybe);
            let first_kind = d.fixes.iter().find(ok).map(|f| f.applicability);
            d.fixes.iter().filter(move |f| Some(f.applicability) == first_kind && ok(f))
        })
        .collect()
}

/// Check and fix repeatedly. Returns the new source, what was applied, and the
/// diagnostics that remain.
pub fn apply_all(src: &str, maybe: bool) -> (String, Vec<Applied>, Vec<Diagnostic>) {
    let mut cur = src.to_string();
    let mut applied = Vec::new();
    for _ in 0..MAX_ROUNDS {
        let diags = crate::check(&cur);
        let fixes = selected(&diags, maybe);
        if fixes.is_empty() {
            return (cur, applied, diags);
        }
        let (next, n) = apply(&cur, &fixes);
        if n == 0 || next == cur {
            return (cur, applied, diags);
        }
        for d in &diags {
            if let Some(f) = d.fixes.iter().find(|f| fixes.iter().any(|g| std::ptr::eq(*g, *f))) {
                applied.push(Applied { code: d.code, label: f.label.clone() });
            }
        }
        cur = next;
    }
    let diags = crate::check(&cur);
    (cur, applied, diags)
}

/// Write `contents` to `path` atomically: a new file in the same directory,
/// then a rename, so a crash never leaves a half-written source file. A symlink
/// is followed, and the file keeps its permissions.
pub fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    let real = std::fs::canonicalize(path)?;
    let dir = real.parent().ok_or_else(|| std::io::Error::other("file has no parent directory"))?;
    let name = real.file_name().ok_or_else(|| std::io::Error::other("no file name"))?.to_string_lossy();
    let perms = std::fs::metadata(&real)?.permissions();
    let tmp = dir.join(format!(".{name}.om-fix-{:016x}", crate::random_u64()));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
        std::fs::set_permissions(&tmp, perms)?;
        std::fs::rename(&tmp, &real)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use om_lexer::Span;

    fn fix(start: usize, end: usize, text: &str) -> Fix {
        Fix { applicability: Applicability::Auto, span: Span::new(start, end), text: text.into(), label: String::new() }
    }

    #[test]
    fn applies_non_overlapping_fixes_back_to_front() {
        let a = fix(0, 1, "X");
        let b = fix(2, 3, "YY");
        let c = fix(2, 4, "zz");
        let (out, n) = apply("abcd", &[&b, &a, &c]);
        assert_eq!((out.as_str(), n), ("XbYYd", 2));
    }

    #[test]
    fn ignores_bad_spans() {
        let a = fix(1, 2, "x");
        let b = fix(9, 10, "y");
        let (out, n) = apply("é", &[&a, &b]);
        assert_eq!((out.as_str(), n), ("é", 0));
    }

    #[test]
    fn fixes_until_clean() {
        let (out, applied, left) =
            apply_all("pub fn main() {\n    let x = 1\n    x = 2\n    println!(\"{}\", x)\n}\n", false);
        assert_eq!(out, "fn main() {\n    let mut x = 1\n    x = 2\n    print(\"{x}\")\n}\n");
        assert!(left.is_empty(), "{left:?}");
        assert_eq!(applied.len(), 3);
    }

    #[test]
    fn writes_atomically_through_symlinks() {
        let dir = crate::TempDir::new().unwrap();
        let real = dir.path().join("a.om");
        std::fs::write(&real, "old").unwrap();
        #[cfg(unix)]
        {
            let link = dir.path().join("b.om");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            write_atomic(&link, "new").unwrap();
            assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
            assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), if cfg!(unix) { 2 } else { 1 });
    }
}
