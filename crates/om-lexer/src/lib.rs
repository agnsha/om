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
    Float(f64),
    Str(String),
    /// A string containing `{expr}` interpolations.
    InterpStr(Vec<StrPart>),
    Ident(String),
    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Return,
    Defer,
    Struct,
    Enum,
    Impl,
    Match,
    Loop,
    Break,
    Continue,
    For,
    In,
    As,
    Const,
    Trait,
    Where,
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
    /// `=>`
    FatArrow,
    /// `::`
    ColonColon,
    Dot,
    /// `..`
    DotDot,
    /// `..=`
    DotDotEq,
    Amp,
    Pipe,
    /// `?`, the error-propagation operator.
    Question,
    LBracket,
    RBracket,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
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
    /// `|>`, the pipeline operator.
    PipeGt,
    Bang,
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TokenKind::*;
        let s = match self {
            Int(n) => return write!(f, "integer `{n}`"),
            Float(n) => return write!(f, "number `{n}`"),
            Str(_) | InterpStr(_) => "string literal",
            Ident(name) => return write!(f, "identifier `{name}`"),
            Fn => "`fn`",
            Let => "`let`",
            Mut => "`mut`",
            If => "`if`",
            Else => "`else`",
            While => "`while`",
            Return => "`return`",
            Defer => "`defer`",
            Struct => "`struct`",
            Enum => "`enum`",
            Impl => "`impl`",
            Match => "`match`",
            Loop => "`loop`",
            Break => "`break`",
            Continue => "`continue`",
            For => "`for`",
            In => "`in`",
            As => "`as`",
            Const => "`const`",
            Trait => "`trait`",
            Where => "`where`",
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
            FatArrow => "`=>`",
            ColonColon => "`::`",
            Dot => "`.`",
            DotDot => "`..`",
            DotDotEq => "`..=`",
            Amp => "`&`",
            Question => "`?`",
            LBracket => "`[`",
            RBracket => "`]`",
            Pipe => "`|`",
            PlusEq => "`+=`",
            MinusEq => "`-=`",
            StarEq => "`*=`",
            SlashEq => "`/=`",
            PercentEq => "`%=`",
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
            PipeGt => "`|>`",
            Bang => "`!`",
            Eof => "end of file",
        };
        f.write_str(s)
    }
}

/// One piece of an interpolated string.
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    /// Literal text.
    Lit(String),
    /// Source code of an embedded expression, and the byte offset where it starts.
    Code(String, usize),
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
        // `1.5` is a float; `1..5` is a range and `1.max()` a method call.
        if self.peek(0) == Some(b'.') && matches!(self.peek(1), Some(b'0'..=b'9')) {
            self.pos += 1;
            while matches!(self.peek(0), Some(b'0'..=b'9' | b'_')) {
                self.pos += 1;
            }
            let text: String = self.src[start..self.pos].chars().filter(|&c| c != '_').collect();
            return Ok(TokenKind::Float(text.parse().unwrap()));
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
        let mut parts = Vec::new();
        let mut s = String::new();
        loop {
            let Some(c) = self.src[self.pos..].chars().next() else {
                return self.err("unterminated string literal", start);
            };
            self.pos += c.len_utf8();
            match c {
                '"' => break,
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
                '{' if self.peek(0) == Some(b'{') => {
                    self.pos += 1;
                    s.push('{');
                }
                '}' if self.peek(0) == Some(b'}') => {
                    self.pos += 1;
                    s.push('}');
                }
                '}' => return self.err("unmatched `}` in string; write `}}` for a literal brace", self.pos - 1),
                '{' => {
                    if !s.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut s)));
                    }
                    let code_start = self.pos;
                    let mut depth = 1;
                    loop {
                        match self.peek(0) {
                            None | Some(b'\n') => {
                                return self.err("unterminated `{` in string interpolation", code_start - 1)
                            }
                            Some(b'"') => return self.err(
                                "unterminated `{` in string interpolation (strings cannot be nested inside `{...}`)",
                                code_start - 1,
                            ),
                            Some(b'{') => depth += 1,
                            Some(b'}') => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        self.pos += 1;
                    }
                    let code = self.src[code_start..self.pos].to_string();
                    self.pos += 1; // closing brace
                    if code.trim().is_empty() {
                        return self.err("empty `{}` in string; write `{{}}` for literal braces", code_start - 1);
                    }
                    parts.push(StrPart::Code(code, code_start));
                }
                c => s.push(c),
            }
        }
        if parts.is_empty() {
            return Ok(TokenKind::Str(s));
        }
        if !s.is_empty() {
            parts.push(StrPart::Lit(s));
        }
        Ok(TokenKind::InterpStr(parts))
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
            "defer" => TokenKind::Defer,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "impl" => TokenKind::Impl,
            "match" => TokenKind::Match,
            "loop" => TokenKind::Loop,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "as" => TokenKind::As,
            "const" => TokenKind::Const,
            "trait" => TokenKind::Trait,
            "where" => TokenKind::Where,
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
            (b'.', Some(b'.')) if self.peek(2) == Some(b'=') => (DotDotEq, 3),
            (b'.', Some(b'.')) => (DotDot, 2),
            (b'.', _) => (Dot, 1),
            (b':', Some(b':')) => (ColonColon, 2),
            (b'=', Some(b'>')) => (FatArrow, 2),
            (b'+', Some(b'=')) => (PlusEq, 2),
            (b'-', Some(b'=')) => (MinusEq, 2),
            (b'*', Some(b'=')) => (StarEq, 2),
            (b'/', Some(b'=')) => (SlashEq, 2),
            (b'%', Some(b'=')) => (PercentEq, 2),
            (b'-', Some(b'>')) => (Arrow, 2),
            (b'=', Some(b'=')) => (EqEq, 2),
            (b'!', Some(b'=')) => (NotEq, 2),
            (b'<', Some(b'=')) => (LtEq, 2),
            (b'>', Some(b'=')) => (GtEq, 2),
            (b'&', Some(b'&')) => (AndAnd, 2),
            (b'|', Some(b'|')) => (OrOr, 2),
            (b'|', Some(b'>')) => (PipeGt, 2),
            (b'&', _) => (Amp, 1),
            (b'?', _) => (Question, 1),
            (b'[', _) => (LBracket, 1),
            (b']', _) => (RBracket, 1),
            (b'|', _) => (Pipe, 1),
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
    fn interpolation() {
        assert_eq!(
            kinds(r#""n = {n + 1}!""#),
            vec![
                InterpStr(vec![
                    StrPart::Lit("n = ".into()),
                    StrPart::Code("n + 1".into(), 6),
                    StrPart::Lit("!".into())
                ]),
                Eof
            ]
        );
        assert_eq!(kinds(r#""{{x}}""#), vec![Str("{x}".into()), Eof]);
        assert!(lex(r#""{}""#).unwrap_err().message.contains("empty"));
        assert!(lex(r#""{x""#).unwrap_err().message.contains("unterminated"));
    }

    #[test]
    fn ranges_floats_and_paths() {
        assert_eq!(
            kinds("0..10 1..=2 3.25 a.b A::B x += 1 =>"),
            vec![
                Int(0),
                DotDot,
                Int(10),
                Int(1),
                DotDotEq,
                Int(2),
                Float(3.25),
                Ident("a".into()),
                Dot,
                Ident("b".into()),
                Ident("A".into()),
                ColonColon,
                Ident("B".into()),
                Ident("x".into()),
                PlusEq,
                Int(1),
                FatArrow,
                Eof
            ]
        );
    }

    #[test]
    fn pipe_and_defer() {
        assert_eq!(kinds("defer x |> f"), vec![Defer, Ident("x".into()), PipeGt, Ident("f".into()), Eof]);
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
