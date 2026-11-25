//! `om guide`: a compact language card for agents, shipped inside the compiler
//! so it always matches the version in use. Tests keep it under ~4k tokens and
//! check that every example in it compiles.

use crate::codes::CODES;

/// The guide text (`docs/guide.md`).
pub const GUIDE: &str = include_str!("../../../docs/guide.md");

/// Rough token budget for the guide. A model reads it once per session.
pub const TOKEN_BUDGET: usize = 4000;

/// `om guide --codes`: one line per error code.
pub fn codes() -> String {
    let mut out = String::new();
    for c in CODES {
        out.push_str(&format!("{} {}\n", c.code, c.title));
    }
    out
}

/// The ```om code blocks in the guide.
pub fn examples(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        match (&mut cur, line.trim()) {
            (None, "```om") => cur = Some(String::new()),
            (Some(_), "```") => out.push(cur.take().unwrap_or_default()),
            (Some(c), _) => {
                c.push_str(line);
                c.push('\n');
            }
            (None, _) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_fits_the_token_budget() {
        let n = crate::tokens::count(GUIDE);
        assert!(n <= TOKEN_BUDGET, "the guide is ~{n} tokens; keep it under {TOKEN_BUDGET}");
    }

    #[test]
    fn guide_examples_compile_and_run() {
        let ex = examples(GUIDE);
        assert!(ex.len() >= 2);
        for src in ex {
            let d = crate::check(&src);
            assert!(d.is_empty(), "guide example does not compile:\n{src}\n{d:?}");
        }
    }

    #[test]
    fn guide_names_real_codes() {
        for word in GUIDE.split(|c: char| !c.is_ascii_alphanumeric()) {
            if word.len() == 5
                && (word.starts_with('E') || word.starts_with('W'))
                && word[1..].chars().all(|c| c.is_ascii_digit())
            {
                if word.ends_with("xx") {
                    continue;
                }
                assert!(crate::codes::find(word).is_some(), "the guide mentions unknown code {word}");
            }
        }
    }
}
