//! Just enough JSON output for diagnostics and tool results. The compiler takes
//! no third-party dependencies for this, which keeps its supply chain small.

use std::fmt::Write;

/// Append `s` as a JSON string literal.
pub fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Escape every control character, and the line separators that break JavaScript.
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A JSON object written field by field: `Obj::new().str("a", "x").num("b", 1).finish()`.
pub struct Obj {
    out: String,
    first: bool,
}

impl Default for Obj {
    fn default() -> Self {
        Self::new()
    }
}

impl Obj {
    pub fn new() -> Obj {
        Obj { out: String::from("{"), first: true }
    }

    fn key(&mut self, k: &str) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        string(&mut self.out, k);
        self.out.push(':');
    }

    pub fn str(mut self, k: &str, v: &str) -> Obj {
        self.key(k);
        string(&mut self.out, v);
        self
    }

    pub fn num(mut self, k: &str, v: impl Into<i128>) -> Obj {
        self.key(k);
        let _ = write!(self.out, "{}", v.into());
        self
    }

    pub fn bool(mut self, k: &str, v: bool) -> Obj {
        self.key(k);
        self.out.push_str(if v { "true" } else { "false" });
        self
    }

    /// A field whose value is already JSON.
    pub fn raw(mut self, k: &str, json: &str) -> Obj {
        self.key(k);
        self.out.push_str(json);
        self
    }

    pub fn finish(mut self) -> String {
        self.out.push('}');
        self.out
    }
}

/// `[a,b,c]` from already-encoded items.
pub fn array(items: impl IntoIterator<Item = String>) -> String {
    let items: Vec<String> = items.into_iter().collect();
    format!("[{}]", items.join(","))
}

/// `["a","b"]` from plain strings.
pub fn strings<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    array(items.into_iter().map(|s| {
        let mut o = String::new();
        string(&mut o, s);
        o
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_everything_dangerous() {
        let mut o = String::new();
        string(&mut o, "a\"b\\c\nd\u{1}e\u{2028}");
        assert_eq!(o, "\"a\\\"b\\\\c\\nd\\u0001e\\u2028\"");
    }

    #[test]
    fn builds_objects() {
        let o = Obj::new().str("code", "E0200").num("line", 3).bool("ok", false).raw("x", "[]").finish();
        assert_eq!(o, r#"{"code":"E0200","line":3,"ok":false,"x":[]}"#);
    }
}
