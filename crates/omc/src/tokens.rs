//! A rough LLM token count, for tracking how many tokens Om programs cost
//! (`om tokens`, and the budget test over `bench/tokens`).
//!
//! It imitates byte-pair tokenizers on code: a short word is one token, a long
//! one a token per ~4 characters, a run of spaces or a newline plus indentation
//! is one token, each digit group of up to 3 is one token, and common
//! multi-character operators are one token. It is an approximation, good for
//! comparing programs and languages, not for billing.

pub fn count(src: &str) -> usize {
    let chars: Vec<char> = src.chars().collect();
    let mut n = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            // snake_case pieces tokenize separately.
            for part in chars[start..i].split(|&c| c == '_') {
                let len = part.len();
                n += if len == 0 {
                    0
                } else if len <= 6 {
                    1
                } else {
                    len.div_ceil(4)
                };
            }
            n += chars[start..i].iter().filter(|&&c| c == '_').count().saturating_sub(0) / 2;
        } else if c.is_ascii_digit() {
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            n += (i - start).div_ceil(3);
        } else if c == '\n' {
            // A newline and the indentation after it.
            i += 1;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
            n += 1;
        } else if c.is_whitespace() {
            // Single spaces merge into the next word.
            while i < chars.len() && chars[i].is_whitespace() && chars[i] != '\n' {
                i += 1;
            }
            if i - start > 1 {
                n += 1;
            }
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let ops = ["->", "=>", "::", "==", "!=", "<=", ">=", "&&", "||", "|>", "..", "+=", "-=", "//", "/*", "*/"];
            i += if ops.contains(&two.as_str()) { 2 } else { 1 };
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::count;

    #[test]
    fn counts_plausibly() {
        assert_eq!(count(""), 0);
        assert_eq!(count("fn main() {}"), 6);
        assert!(count("let x = 1") < count("let mut x: i64 = 1;"));
        assert!(count("internationalization") > 2);
    }
}
