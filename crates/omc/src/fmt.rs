//! `om fmt`: one canonical layout, so diffs stay small and agents can edit by
//! exact string match.
//!
//! The formatter only changes whitespace at the start and end of lines and
//! blank lines, so it can never change what a program means:
//!
//! * indentation is 4 spaces per open bracket, with one extra level for
//!   continuation lines that start with `.`, `|>` or a binary operator;
//! * trailing whitespace goes, runs of blank lines become one, and the file
//!   ends with exactly one newline.
//!
//! Lines inside multi-line strings and block comments are left untouched.

/// Format `src`.
pub fn format(src: &str) -> String {
    let mut out = String::new();
    let mut depth: i64 = 0;
    let mut state = State::Code;
    let mut blank_run = 0;
    for raw in src.lines() {
        if state != State::Code {
            // Inside a string or block comment that started on an earlier line.
            out.push_str(raw);
            out.push('\n');
            let (d, s) = scan(raw, state);
            depth += d;
            state = s;
            blank_run = 0;
            continue;
        }
        let line = raw.trim();
        if line.is_empty() {
            blank_run += 1;
            continue;
        }
        if blank_run > 0 && !out.is_empty() && !out.ends_with("{\n") {
            out.push('\n');
        }
        blank_run = 0;
        let (d, s) = scan(line, State::Code);
        // Closing brackets at the start of the line dedent that line.
        let leading_closers = line.chars().take_while(|c| matches!(c, '}' | ')' | ']')).count() as i64;
        let continuation = [".", "|>", "&&", "||", "+ ", "- ", "* ", "/ ", "? "]
            .iter()
            .any(|p| line.starts_with(p) && !line.starts_with(".."));
        let level = (depth - leading_closers).max(0) + i64::from(continuation);
        // A line ending inside a string keeps its trailing spaces: they are part of the string.
        let text = if s == State::Str { raw.trim_start() } else { line };
        out.push_str(&"    ".repeat(level as usize));
        out.push_str(text);
        out.push('\n');
        depth = (depth + d).max(0);
        state = s;
    }
    if out.is_empty() {
        return out;
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Code,
    Str,
    Comment,
}

/// Net bracket depth change over `line`, and the state at its end.
fn scan(line: &str, mut state: State) -> (i64, State) {
    let mut d = 0;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match state {
            State::Str => match c {
                '\\' => {
                    chars.next();
                }
                '"' => state = State::Code,
                _ => {}
            },
            State::Comment => {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    state = State::Code;
                }
            }
            State::Code => match c {
                '"' => state = State::Str,
                '\'' => {
                    // A char literal like '{' or '\''; a label like 'outer has no closing quote.
                    let rest: String = chars.clone().take(12).collect();
                    let lit_len = if let Some(esc) = rest.strip_prefix('\\') {
                        esc.find('\'').map(|i| i + 2)
                    } else {
                        rest.chars()
                            .nth(1)
                            .filter(|&q| q == '\'')
                            .map(|_| rest.chars().next().map_or(1, char::len_utf8) + 1)
                    };
                    if let Some(n) = lit_len {
                        let mut taken = 0;
                        while taken < n {
                            match chars.next() {
                                Some(ch) => taken += ch.len_utf8(),
                                None => break,
                            }
                        }
                    }
                }
                '/' if chars.peek() == Some(&'/') => break,
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    state = State::Comment;
                }
                '{' | '(' | '[' => d += 1,
                '}' | ')' | ']' => d -= 1,
                _ => {}
            },
        }
    }
    (d, state)
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn indents_by_brackets() {
        let src = "fn main() {\nlet x = 1   \n\n\n  if x > 0 {\n print(\"{x}\")\n      }\n}";
        assert_eq!(format(src), "fn main() {\n    let x = 1\n\n    if x > 0 {\n        print(\"{x}\")\n    }\n}\n");
    }

    #[test]
    fn leaves_strings_comments_and_chars_alone() {
        let src = "fn main() {\n    let c = '{'\n    /* a {\n   b */\n    print(\"a {{ b\")\n}\n";
        assert_eq!(format(src), src);
    }

    #[test]
    fn is_idempotent_on_the_examples() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "om") {
                let src = std::fs::read_to_string(&p).unwrap();
                let once = format(&src);
                assert_eq!(format(&once), once, "{}", p.display());
                assert_eq!(once, src, "{} is not formatted; run `om fmt` on it", p.display());
            }
        }
    }

    #[test]
    fn continuation_lines_get_one_level() {
        let src = "fn main() {\n    let x = 1\n    |> double\n    print(x)\n}\n";
        assert_eq!(format(src), "fn main() {\n    let x = 1\n        |> double\n    print(x)\n}\n");
    }
}
