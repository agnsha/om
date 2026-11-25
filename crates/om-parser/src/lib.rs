//! Recursive-descent parser for Om.
//!
//! Grammar (phase 0):
//!
//! ```text
//! program  = function*
//! function = "fn" IDENT "(" (param ("," param)*)? ")" ("->" type)? block
//! param    = IDENT ":" type
//! block    = "{" stmt* expr? "}"
//! stmt     = "let" "mut"? IDENT (":" type)? "=" expr ";"?
//!          | IDENT "=" expr ";"?
//!          | "while" expr block
//!          | "return" expr? ";"?
//!          | expr ";"?
//! expr     = precedence climbing over || && comparisons + - * / % and unary - !
//! primary  = INT | STR | true | false | IDENT | IDENT "(" args ")" | "(" expr ")"
//!          | "if" expr block ("else" (block | if))? | block
//! ```
//!
//! Semicolons are optional. A block's last expression without `;` is its value.

pub mod ast;

use ast::*;
use om_lexer::{lex, LexError, Span, Token, TokenKind as T};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError { message: e.message, span: e.span }
    }
}

type PResult<T> = Result<T, ParseError>;

/// Lex and parse a whole Om source file.
pub fn parse(src: &str) -> PResult<Program> {
    let tokens = lex(src)?;
    let mut p = Parser { tokens, pos: 0 };
    let mut functions = Vec::new();
    while !p.at(&T::Eof) {
        functions.push(p.function()?);
    }
    Ok(Program { functions })
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_at(&self, n: usize) -> &T {
        &self.tokens[(self.pos + n).min(self.tokens.len() - 1)].kind
    }

    fn at(&self, k: &T) -> bool {
        &self.peek().kind == k
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if t.kind != T::Eof {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, k: &T) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn prev_span(&self) -> Span {
        self.tokens[self.pos.saturating_sub(1)].span
    }

    fn error<X>(&self, expected: &str) -> PResult<X> {
        let t = self.peek();
        Err(ParseError { message: format!("expected {expected}, found {}", t.kind), span: t.span })
    }

    fn expect(&mut self, k: T) -> PResult<Span> {
        if self.at(&k) {
            Ok(self.bump().span)
        } else {
            self.error(&k.to_string())
        }
    }

    fn ident(&mut self) -> PResult<(String, Span)> {
        match &self.peek().kind {
            T::Ident(name) => {
                let name = name.clone();
                Ok((name, self.bump().span))
            }
            _ => self.error("a name"),
        }
    }

    fn ty(&mut self) -> PResult<TypeExpr> {
        let (name, span) = self.ident()?;
        Ok(TypeExpr { name, span })
    }

    fn function(&mut self) -> PResult<Function> {
        let start = self.expect(T::Fn)?;
        let (name, _) = self.ident()?;
        self.expect(T::LParen)?;
        let mut params = Vec::new();
        while !self.at(&T::RParen) {
            let (pname, pspan) = self.ident()?;
            self.expect(T::Colon)?;
            let ty = self.ty()?;
            params.push(Param { name: pname, span: pspan.to(ty.span), ty });
            if !self.eat(&T::Comma) {
                break;
            }
        }
        self.expect(T::RParen)?;
        let ret = if self.eat(&T::Arrow) { Some(self.ty()?) } else { None };
        let body = self.block()?;
        Ok(Function { name, params, ret, span: start.to(body.span), body })
    }

    fn block(&mut self) -> PResult<Block> {
        let start = self.expect(T::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail = None;
        while !self.at(&T::RBrace) {
            if self.at(&T::Eof) {
                return self.error("`}`");
            }
            let stmt = self.stmt()?;
            let had_semi = self.eat(&T::Semi);
            match stmt {
                Stmt::Expr(e) if !had_semi && self.at(&T::RBrace) => tail = Some(Box::new(e)),
                s => stmts.push(s),
            }
        }
        let end = self.expect(T::RBrace)?;
        Ok(Block { stmts, tail, span: start.to(end) })
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.peek().span;
        match self.peek().kind {
            T::Let => {
                self.bump();
                let mutable = self.eat(&T::Mut);
                let (name, _) = self.ident()?;
                let ty = if self.eat(&T::Colon) { Some(self.ty()?) } else { None };
                self.expect(T::Eq)?;
                let value = self.expr()?;
                Ok(Stmt::Let { name, mutable, ty, span: start.to(value.span), value })
            }
            T::While => {
                self.bump();
                let cond = self.expr()?;
                let body = self.block()?;
                Ok(Stmt::While { cond, span: start.to(body.span), body })
            }
            T::Return => {
                self.bump();
                let value = if matches!(self.peek().kind, T::Semi | T::RBrace) { None } else { Some(self.expr()?) };
                Ok(Stmt::Return { value, span: start.to(self.prev_span()) })
            }
            T::Ident(_) if self.peek_at(1) == &T::Eq => {
                let (name, _) = self.ident()?;
                self.bump(); // `=`
                let value = self.expr()?;
                Ok(Stmt::Assign { name, span: start.to(value.span), value })
            }
            _ => Ok(Stmt::Expr(self.expr()?)),
        }
    }

    fn expr(&mut self) -> PResult<Expr> {
        self.binary(0)
    }

    /// Precedence climbing. Higher numbers bind tighter.
    fn binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.unary()?;
        loop {
            let (op, prec) = match self.peek().kind {
                T::OrOr => (BinOp::Or, 1),
                T::AndAnd => (BinOp::And, 2),
                T::EqEq => (BinOp::Eq, 3),
                T::NotEq => (BinOp::Ne, 3),
                T::Lt => (BinOp::Lt, 3),
                T::LtEq => (BinOp::Le, 3),
                T::Gt => (BinOp::Gt, 3),
                T::GtEq => (BinOp::Ge, 3),
                T::Plus => (BinOp::Add, 4),
                T::Minus => (BinOp::Sub, 4),
                T::Star => (BinOp::Mul, 5),
                T::Slash => (BinOp::Div, 5),
                T::Percent => (BinOp::Rem, 5),
                _ => return Ok(lhs),
            };
            if prec < min_prec {
                return Ok(lhs);
            }
            self.bump();
            let rhs = self.binary(prec + 1)?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), span };
        }
    }

    fn unary(&mut self) -> PResult<Expr> {
        let start = self.peek().span;
        let op = match self.peek().kind {
            T::Minus => UnOp::Neg,
            T::Bang => UnOp::Not,
            _ => return self.primary(),
        };
        self.bump();
        let inner = self.unary()?;
        let span = start.to(inner.span);
        Ok(Expr { kind: ExprKind::Unary(op, Box::new(inner)), span })
    }

    fn primary(&mut self) -> PResult<Expr> {
        let tok = self.peek().clone();
        let span = tok.span;
        let kind = match tok.kind {
            T::Int(n) => {
                self.bump();
                ExprKind::Int(n)
            }
            T::Str(s) => {
                self.bump();
                ExprKind::Str(s)
            }
            T::True | T::False => {
                self.bump();
                ExprKind::Bool(tok.kind == T::True)
            }
            T::Ident(name) => {
                self.bump();
                if self.eat(&T::LParen) {
                    let mut args = Vec::new();
                    while !self.at(&T::RParen) {
                        args.push(self.expr()?);
                        if !self.eat(&T::Comma) {
                            break;
                        }
                    }
                    let end = self.expect(T::RParen)?;
                    return Ok(Expr { kind: ExprKind::Call(name, args), span: span.to(end) });
                }
                ExprKind::Var(name)
            }
            T::LParen => {
                self.bump();
                let mut inner = self.expr()?;
                let end = self.expect(T::RParen)?;
                inner.span = span.to(end);
                return Ok(inner);
            }
            T::If => return self.if_expr(),
            T::LBrace => {
                let b = self.block()?;
                return Ok(Expr { span: b.span, kind: ExprKind::Block(b) });
            }
            _ => return self.error("an expression"),
        };
        Ok(Expr { kind, span })
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(T::If)?;
        let cond = self.expr()?;
        let then = self.block()?;
        let els = if self.eat(&T::Else) {
            if self.at(&T::If) {
                // `else if` becomes an else block holding one if-expression.
                let nested = self.if_expr()?;
                Some(Block { span: nested.span, stmts: vec![], tail: Some(Box::new(nested)) })
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        let end = els.as_ref().map_or(then.span, |b| b.span);
        Ok(Expr { kind: ExprKind::If(Box::new(cond), then, els), span: start.to(end) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world() {
        let p = parse(r#"fn main() { print("Hello, Om!") }"#).unwrap();
        assert_eq!(p.functions.len(), 1);
        let f = &p.functions[0];
        assert_eq!(f.name, "main");
        let tail = f.body.tail.as_ref().unwrap();
        assert!(matches!(&tail.kind, ExprKind::Call(n, a) if n == "print" && a.len() == 1));
    }

    #[test]
    fn precedence() {
        let p = parse("fn f() -> i64 { 1 + 2 * 3 }").unwrap();
        let tail = p.functions[0].body.tail.as_ref().unwrap();
        let ExprKind::Binary(BinOp::Add, _, rhs) = &tail.kind else { panic!("{tail:?}") };
        assert!(matches!(rhs.kind, ExprKind::Binary(BinOp::Mul, _, _)));
    }

    #[test]
    fn statements_with_and_without_semicolons() {
        let src = "fn f(n: i64) -> i64 {
            let mut i = 0
            let total: i64 = 0;
            while i < n { i = i + 1 }
            return i
        }";
        let f = &parse(src).unwrap().functions[0];
        assert_eq!(f.params.len(), 1);
        assert_eq!(f.body.stmts.len(), 4);
        assert!(matches!(f.body.stmts[0], Stmt::Let { mutable: true, .. }));
        assert!(matches!(f.body.stmts[3], Stmt::Return { .. }));
    }

    #[test]
    fn else_if_chain() {
        let src = "fn f(x: i64) -> i64 { if x < 0 { 0 } else if x < 10 { 1 } else { 2 } }";
        assert!(parse(src).is_ok());
    }

    #[test]
    fn reports_errors_with_location() {
        let err = parse("fn main( { }").unwrap_err();
        assert_eq!(err.message, "expected a name, found `{`");
        assert_eq!(err.span.start, 9);
    }
}
