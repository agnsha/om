//! Lexer for the Om language.
//!
//! Turns source text into a flat list of [`Token`]s, each carrying the byte
//! [`Span`] it came from so later stages can point errors at exact source.

use std::fmt;

/// A half-open byte range `start..end` in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals and names
    Int(i64),
    Str(String),
    Ident(String),
    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Return,
    True,
    False,
    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Semi,
    Arrow,
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    Bang,
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TokenKind::*;
        let s = match self {
            Int(n) => return write!(f, "integer `{n}`"),
            Str(_) => "string literal",
            Ident(name) => return write!(f, "identifier `{name}`"),
            Fn => "`fn`",
            Let => "`let`",
            Mut => "`mut`",
            If => "`if`",
            Else => "`else`",
            While => "`while`",
            Return => "`return`",
            True => "`true`",
            False => "`false`",
            LParen => "`(`",
            RParen => "`)`",
            LBrace => "`{`",
            RBrace => "`}`",
            Comma => "`,`",
            Colon => "`:`",
            Semi => "`;`",
            Arrow => "`->`",
            Plus => "`+`",
            Minus => "`-`",
            Star => "`*`",
            Slash => "`/`",
            Percent => "`%`",
            Eq => "`=`",
            EqEq => "`==`",
            NotEq => "`!=`",
            Lt => "`<`",
            LtEq => "`<=`",
            Gt => "`>`",
            GtEq => "`>=`",
            AndAnd => "`&&`",
            OrOr => "`||`",
            Bang => "`!`",
            Eof => "end of file",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LexError {}

/// Tokenize a whole source file. The returned list always ends with [`TokenKind::Eof`].
pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    Lexer { src, bytes: src.as_bytes(), pos: 0 }.run()
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn run(mut self) -> Result<Vec<Token>, LexError> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia()?;
            let start = self.pos;
            let Some(&c) = self.bytes.get(self.pos) else {
                out.push(Token { kind: TokenKind::Eof, span: Span::new(start, start) });
                return Ok(out);
            };
            let kind = match c {
                b'0'..=b'9' => self.number()?,
                b'"' => self.string()?,
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.ident(),
                _ => self.punct()?,
            };
            out.push(Token { kind, span: Span::new(start, self.pos) });
        }
    }

    fn peek(&self, off: usize) -> Option<u8> {
        self.bytes.get(self.pos + off).copied()
    }

    fn err<T>(&self, message: impl Into<String>, start: usize) -> Result<T, LexError> {
        Err(LexError { message: message.into(), span: Span::new(start, self.pos.max(start + 1)) })
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek(0) {
                Some(b' ' | b'\t' | b'\r' | b'\n') => self.pos += 1,
                Some(b'/') if self.peek(1) == Some(b'/') => {
                    while let Some(c) = self.peek(0) {
                        if c == b'\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                Some(b'/') if self.peek(1) == Some(b'*') => {
                    let start = self.pos;
                    self.pos += 2;
                    loop {
                        match self.peek(0) {
                            None => return self.err("unterminated block comment", start),
                            Some(b'*') if self.peek(1) == Some(b'/') => {
                                self.pos += 2;
                                break;
                            }
                            _ => self.pos += 1,
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn number(&mut self) -> Result<TokenKind, LexError> {
        let start = self.pos;
        while matches!(self.peek(0), Some(b'0'..=b'9' | b'_')) {
            self.pos += 1;
        }
        let text: String = self.src[start..self.pos].chars().filter(|&c| c != '_').collect();
        match text.parse::<i64>() {
            Ok(n) => Ok(TokenKind::Int(n)),
            Err(_) => self.err(format!("integer literal `{text}` does not fit in 64 bits"), start),
        }
    }

    fn string(&mut self) -> Result<TokenKind, LexError> {
        let start = self.pos;
        self.pos += 1; // opening quote
        let mut s = String::new();
        loop {
            let Some(c) = self.src[self.pos..].chars().next() else {
                return self.err("unterminated string literal", start);
            };
            self.pos += c.len_utf8();
            match c {
                '"' => return Ok(TokenKind::Str(s)),
                '\\' => {
                    let esc = self.peek(0);
                    self.pos += 1;
                    s.push(match esc {
                        Some(b'n') => '\n',
                        Some(b't') => '\t',
                        Some(b'r') => '\r',
                        Some(b'0') => '\0',
                        Some(b'\\') => '\\',
                        Some(b'"') => '"',
                        _ => return self.err("unknown escape sequence", self.pos - 2),
                    });
                }
                c => s.push(c),
            }
        }
    }

    fn ident(&mut self) -> TokenKind {
        let start = self.pos;
        while matches!(self.peek(0), Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')) {
            self.pos += 1;
        }
        match &self.src[start..self.pos] {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            name => TokenKind::Ident(name.to_string()),
        }
    }

    fn punct(&mut self) -> Result<TokenKind, LexError> {
        use TokenKind::*;
        let start = self.pos;
        let two = (self.peek(0).unwrap(), self.peek(1));
        let (kind, len) = match two {
            (b'-', Some(b'>')) => (Arrow, 2),
            (b'=', Some(b'=')) => (EqEq, 2),
            (b'!', Some(b'=')) => (NotEq, 2),
            (b'<', Some(b'=')) => (LtEq, 2),
            (b'>', Some(b'=')) => (GtEq, 2),
            (b'&', Some(b'&')) => (AndAnd, 2),
            (b'|', Some(b'|')) => (OrOr, 2),
            (b'(', _) => (LParen, 1),
            (b')', _) => (RParen, 1),
            (b'{', _) => (LBrace, 1),
            (b'}', _) => (RBrace, 1),
            (b',', _) => (Comma, 1),
            (b':', _) => (Colon, 1),
            (b';', _) => (Semi, 1),
            (b'+', _) => (Plus, 1),
            (b'-', _) => (Minus, 1),
            (b'*', _) => (Star, 1),
            (b'/', _) => (Slash, 1),
            (b'%', _) => (Percent, 1),
            (b'=', _) => (Eq, 1),
            (b'<', _) => (Lt, 1),
            (b'>', _) => (Gt, 1),
            (b'!', _) => (Bang, 1),
            _ => {
                let ch = self.src[self.pos..].chars().next().unwrap();
                self.pos += ch.len_utf8();
                return self.err(format!("unexpected character `{ch}`"), start);
            }
        };
        self.pos += len;
        Ok(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn hello_world() {
        assert_eq!(
            kinds(r#"fn main() { print("Hello") }"#),
            vec![
                Fn,
                Ident("main".into()),
                LParen,
                RParen,
                LBrace,
                Ident("print".into()),
                LParen,
                Str("Hello".into()),
                RParen,
                RBrace,
                Eof
            ]
        );
    }

    #[test]
    fn operators_and_numbers() {
        assert_eq!(
            kinds("a <= 1_000 && b != -2 -> x"),
            vec![
                Ident("a".into()),
                LtEq,
                Int(1000),
                AndAnd,
                Ident("b".into()),
                NotEq,
                Minus,
                Int(2),
                Arrow,
                Ident("x".into()),
                Eof
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(kinds("// line\n/* block\n */ let"), vec![Let, Eof]);
    }

    #[test]
    fn string_escapes() {
        assert_eq!(kinds(r#""a\n\"b\"""#), vec![Str("a\n\"b\"".into()), Eof]);
    }

    #[test]
    fn spans_point_at_source() {
        let toks = lex("let  answer").unwrap();
        assert_eq!(toks[1].span, Span::new(5, 11));
    }

    #[test]
    fn errors() {
        assert!(lex("\"open").unwrap_err().message.contains("unterminated"));
        assert!(lex("let @").unwrap_err().message.contains("unexpected character"));
        assert!(lex("99999999999999999999").unwrap_err().message.contains("64 bits"));
    }
}
