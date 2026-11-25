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
    /// An integer literal and its optional type suffix (`5u8`).
    Int(u64, Option<String>),
    /// A float literal and its optional type suffix (`1.5f32`).
    Float(f64, Option<String>),
    /// `'a'`
    Char(char),
    /// `'outer`, a loop label.
    Label(String),
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
    AmpEq,
    PipeEq,
    CaretEq,
    ShlEq,
    ShrEq,
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
    Caret,
    Shl,
    Shr,
    Bang,
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TokenKind::*;
        let s = match self {
            Int(n, _) => return write!(f, "integer `{n}`"),
            Char(c) => return write!(f, "character {c:?}"),
            Label(l) => return write!(f, "label `'{l}`"),
            Float(n, _) => return write!(f, "number `{n}`"),
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
            AmpEq => "`&=`",
            PipeEq => "`|=`",
            CaretEq => "`^=`",
            ShlEq => "`<<=`",
            ShrEq => "`>>=`",
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
            Caret => "`^`",
            Shl => "`<<`",
            Shr => "`>>`",
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
                b'\'' => self.quote()?,
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
        let radix = match (self.peek(0), self.peek(1)) {
            (Some(b'0'), Some(b'x')) => 16,
            (Some(b'0'), Some(b'o')) => 8,
            (Some(b'0'), Some(b'b')) => 2,
            _ => 10,
        };
        if radix != 10 {
            self.pos += 2;
        }
        let digits_start = self.pos;
        while let Some(c) = self.peek(0) {
            if c == b'_' || (c as char).is_digit(radix) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let digits: String = self.src[digits_start..self.pos].chars().filter(|&c| c != '_').collect();
        if digits.is_empty() {
            return self.err("missing digits after the number prefix", start);
        }
        let mut is_float = false;
        if radix == 10 {
            // `1.5` is a float; `1..5` is a range and `1.max()` a method call.
            if self.peek(0) == Some(b'.') && matches!(self.peek(1), Some(b'0'..=b'9')) {
                is_float = true;
                self.pos += 1;
                while matches!(self.peek(0), Some(b'0'..=b'9' | b'_')) {
                    self.pos += 1;
                }
            }
            // Exponent: `1e9`, `2.5e-3`.
            let exp_digit = |o: usize| matches!(self.peek(o), Some(b'0'..=b'9'));
            if matches!(self.peek(0), Some(b'e' | b'E'))
                && (exp_digit(1) || (matches!(self.peek(1), Some(b'+' | b'-')) && exp_digit(2)))
            {
                is_float = true;
                self.pos += 2;
                while matches!(self.peek(0), Some(b'0'..=b'9' | b'_')) {
                    self.pos += 1;
                }
            }
        }
        let text: String = self.src[start..self.pos].chars().filter(|&c| c != '_').collect();
        let suffix_start = self.pos;
        while matches!(self.peek(0), Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')) {
            self.pos += 1;
        }
        let suffix = &self.src[suffix_start..self.pos];
        let suffix = match suffix {
            "" => None,
            "f32" | "f64" if radix == 10 => {
                return Ok(TokenKind::Float(text.parse().unwrap(), Some(suffix.to_string())));
            }
            "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" if !is_float => {
                Some(suffix.to_string())
            }
            _ => return self.err(format!("invalid suffix `{suffix}` on a number"), suffix_start),
        };
        if is_float {
            return Ok(TokenKind::Float(text.parse().unwrap(), None));
        }
        match u64::from_str_radix(&digits, radix) {
            Ok(n) => Ok(TokenKind::Int(n, suffix)),
            Err(_) => self.err(format!("integer literal `{text}` does not fit in 64 bits"), start),
        }
    }

    /// `'a'` (a character) or `'outer` (a loop label).
    fn quote(&mut self) -> Result<TokenKind, LexError> {
        let start = self.pos;
        self.pos += 1;
        let Some(c) = self.src[self.pos..].chars().next() else {
            return self.err("unterminated character literal", start);
        };
        let value = if c == '\\' {
            let esc = self.peek(1);
            self.pos += 2;
            match esc {
                Some(b'n') => '\n',
                Some(b't') => '\t',
                Some(b'r') => '\r',
                Some(b'0') => '\0',
                Some(b'\\') => '\\',
                Some(b'\'') => '\'',
                Some(b'"') => '"',
                Some(b'u') if self.peek(0) == Some(b'{') => {
                    let close = self.src[self.pos..].find('}').map(|i| self.pos + i);
                    let code = close.and_then(|e| u32::from_str_radix(&self.src[self.pos + 1..e], 16).ok());
                    match (close, code.and_then(char::from_u32)) {
                        (Some(e), Some(ch)) => {
                            self.pos = e + 1;
                            ch
                        }
                        _ => return self.err("invalid unicode escape; write it like `\\u{1F600}`", start),
                    }
                }
                _ => return self.err("unknown escape sequence", start + 1),
            }
        } else if self.src[self.pos + c.len_utf8()..].starts_with('\'') {
            self.pos += c.len_utf8();
            c
        } else if c.is_ascii_alphabetic() || c == '_' {
            let name_start = self.pos;
            while matches!(self.peek(0), Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')) {
                self.pos += 1;
            }
            return Ok(TokenKind::Label(self.src[name_start..self.pos].to_string()));
        } else {
            return self.err("unterminated character literal", start);
        };
        if self.peek(0) != Some(b'\'') {
            return self.err("unterminated character literal", start);
        }
        self.pos += 1;
        Ok(TokenKind::Char(value))
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
            (b'<', Some(b'<')) if self.peek(2) == Some(b'=') => (ShlEq, 3),
            (b'>', Some(b'>')) if self.peek(2) == Some(b'=') => (ShrEq, 3),
            (b'<', Some(b'<')) => (Shl, 2),
            (b'>', Some(b'>')) => (Shr, 2),
            (b'&', Some(b'=')) => (AmpEq, 2),
            (b'|', Some(b'=')) => (PipeEq, 2),
            (b'^', Some(b'=')) => (CaretEq, 2),
            (b'^', _) => (Caret, 1),
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
                Int(1000, None),
                AndAnd,
                Ident("b".into()),
                NotEq,
                Minus,
                Int(2, None),
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
                Int(0, None),
                DotDot,
                Int(10, None),
                Int(1, None),
                DotDotEq,
                Int(2, None),
                Float(3.25, None),
                Ident("a".into()),
                Dot,
                Ident("b".into()),
                Ident("A".into()),
                ColonColon,
                Ident("B".into()),
                Ident("x".into()),
                PlusEq,
                Int(1, None),
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

#[cfg(test)]
mod literal_tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn number_forms_and_suffixes() {
        use TokenKind::*;
        assert_eq!(
            kinds("0xff 0o17 0b1010 5u8 2.5f32 1e3 18446744073709551615"),
            vec![
                Int(255, None),
                Int(15, None),
                Int(10, None),
                Int(5, Some("u8".into())),
                Float(2.5, Some("f32".into())),
                Float(1000.0, None),
                Int(u64::MAX, None),
                Eof
            ]
        );
        assert!(lex("5x").is_err());
        assert!(lex("1.5u8").is_err());
    }

    #[test]
    fn chars_labels_and_bit_operators() {
        use TokenKind::*;
        assert_eq!(
            kinds(r"'a' '\n' '\'' '\u{e9}' 'é' 'outer"),
            vec![Char('a'), Char('\n'), Char('\''), Char('é'), Char('é'), Label("outer".into()), Eof]
        );
        assert_eq!(
            kinds("a << b >>= c ^ d"),
            vec![Ident("a".into()), Shl, Ident("b".into()), ShrEq, Ident("c".into()), Caret, Ident("d".into()), Eof]
        );
    }
}
