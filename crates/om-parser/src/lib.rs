//! Recursive-descent parser for Om.
//!
//! Grammar overview:
//!
//! ```text
//! program  = item*
//! item     = function | struct | enum | impl | const
//! function = "fn" IDENT "(" params ")" ("->" type)? block
//! param    = "&"? "mut"? "self" | "mut"? IDENT ":" ("&" "mut"?)? type ("=" expr)?
//! struct   = "struct" IDENT "{" (IDENT ":" type),* "}"
//! enum     = "enum" IDENT "{" (IDENT ("(" type,* ")")?),* "}"
//! impl     = "impl" type "{" function* "}"
//! const    = "const" IDENT ":" type "=" expr ";"?
//! block    = "{" stmt* expr? "}"
//! stmt     = "let" "mut"? IDENT (":" type)? "=" expr
//!          | place ("=" | "+=" | "-=" | "*=" | "/=" | "%=") expr
//!          | "while" expr block | "for" IDENT "in" expr (".." | "..=") expr block
//!          | "return" expr? | "break" expr? | "continue" | "defer" expr | expr
//! expr     = precedence climbing over |> || && comparisons + - * / % "as" and unary - ! &
//! postfix  = primary ("." IDENT ("(" args ")")?)*
//! primary  = literal | path | path "(" args ")" | path "{" fields "}" | "(" expr ")"
//!          | "if" expr block ("else" (block | if))? | "match" expr "{" arm,* "}"
//!          | "loop" block | block
//! arm      = pattern ("if" expr)? "=>" expr
//! pattern  = alt ("|" alt)*    alt = "_" | IDENT | literal | range | path ("(" pattern,* ")")?
//! ```
//!
//! Semicolons are optional. A block's last expression without `;` is its value.
//! As in Rust, struct literals are not allowed directly in `if`, `while`, `for`
//! and `match` headers, so `if x { ... }` is never read as a struct literal.

pub mod ast;
pub mod visit;

use ast::*;
use om_lexer::{lex, lex_all, LexError, Span, StrPart, Token, TokenKind as T};
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

/// Lex and parse a whole Om source file, stopping at the first error.
pub fn parse(src: &str) -> PResult<Program> {
    let (prog, mut errors) = parse_all(src);
    if errors.is_empty() {
        Ok(prog)
    } else {
        Err(errors.remove(0))
    }
}

/// Deepest nesting of expressions, blocks, types and patterns. Deeper input
/// would overflow the compiler's stack; real code never comes close.
pub const MAX_DEPTH: usize = 256;
/// Most errors one parse reports.
pub const MAX_ERRORS: usize = 200;

/// Lex and parse a whole file, recovering after each error at the next
/// statement or top-level item, so one run reports every syntax error.
/// The program is partial when there are errors.
pub fn parse_all(src: &str) -> (Program, Vec<ParseError>) {
    let (tokens, lex_errors) = lex_all(src);
    let mut errors: Vec<ParseError> = lex_errors.into_iter().map(ParseError::from).collect();
    // Where each token sits on its line, for recovery.
    let mut first_on_line = Vec::with_capacity(tokens.len());
    let mut col0 = Vec::with_capacity(tokens.len());
    let mut prev_end = 0;
    for t in &tokens {
        let start = t.span.start.min(src.len());
        let gap = src.get(prev_end.min(start)..start).unwrap_or("");
        first_on_line.push(prev_end == 0 || gap.contains('\n'));
        col0.push(start == 0 || src[..start].ends_with('\n'));
        prev_end = t.span.end;
    }
    let nl = newlines_before(src, &tokens);
    let mut p =
        Parser { tokens, nl, pos: 0, no_struct: false, label: None, depth: 0, errors: vec![], first_on_line, col0 };
    let mut prog = Program::default();
    while !p.at(&T::Eof) && p.errors.len() < MAX_ERRORS {
        let start = p.pos;
        let item = match p.peek().kind {
            T::Fn => p.function().map(|f| prog.functions.push(f)),
            T::Struct => p.struct_def().map(|s| prog.structs.push(s)),
            T::Enum => p.enum_def().map(|e| prog.enums.push(e)),
            T::Impl => p.impl_block().map(|i| prog.impls.push(i)),
            T::Const => p.const_def().map(|c| prog.consts.push(c)),
            T::Trait => p.trait_def().map(|t| prog.traits.push(t)),
            _ => p.error("`fn`, `struct`, `enum`, `trait`, `impl` or `const`"),
        };
        if let Err(e) = item {
            p.errors.push(e);
            p.sync_item(start);
        }
    }
    errors.append(&mut p.errors);
    errors.sort_by_key(|e| e.span.start);
    errors.truncate(MAX_ERRORS);
    (prog, errors)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// True while parsing an `if`/`while`/`for`/`match` header, where `Name {` starts the body.
    no_struct: bool,
    /// A `'label:` waiting for the loop that follows it.
    label: Option<String>,
    /// Current nesting, checked against [`MAX_DEPTH`].
    depth: usize,
    /// Errors recovered from so far.
    errors: Vec<ParseError>,
    /// Per token: is it the first token on its line, and does it start at column 0?
    first_on_line: Vec<bool>,
    col0: Vec<bool>,
    /// For each token, whether a line break comes before it.
    nl: Vec<bool>,
}

/// Which tokens start a new line. A statement may end at a line break, so an
/// operator that can also start an expression (`*`, `-`, `&`, `[`) does not
/// continue the previous line.
fn newlines_before(src: &str, tokens: &[Token]) -> Vec<bool> {
    let mut prev_end = 0;
    tokens
        .iter()
        .map(|t| {
            let start = t.span.start.min(src.len()).max(prev_end);
            let nl = src.get(prev_end..start).is_some_and(|gap| gap.contains('\n'));
            prev_end = t.span.end.min(src.len()).max(prev_end);
            nl
        })
        .collect()
}

impl Parser {
    /// Is the next token the first on its line?
    fn line_start(&self) -> bool {
        self.nl.get(self.pos).copied().unwrap_or(false)
    }

    /// `for PAT in &list { body }` visits each element by reference, and
    /// `for PAT in list { body }` takes each element out of the list:
    ///
    /// ```text
    /// { let $c = &list; let mut $i = 0
    ///   loop { if $i >= $c.len() { break }; let PAT = &$c[$i]; $i += 1; body } }
    /// { let mut $c = list; $c.reverse()
    ///   loop { match $c.pop() { Some(PAT) => body, None => break } } }
    /// ```
    fn for_each(&mut self, pat: Pattern, iter: Expr, body: Block, label: Option<String>, start: Span) -> PResult<Stmt> {
        if let ExprKind::Ref(true, _) = iter.kind {
            let message = "`for x in &mut list` is not supported yet; loop over indexes: `for i in 0..list.len() { list[i] ... }`";
            return Err(ParseError { message: message.into(), span: iter.span });
        }
        Ok(Stmt::ForEach { pat, span: start.to(body.span), iter, body, label })
    }
}

/// Lower a `for` loop over a list into `let`, `loop` and `match`; see `Parser::for_each`.
/// `id` makes the hidden variable names unique.
pub fn desugar_for_each(pat: Pattern, iter: Expr, body: Block, label: Option<String>, span: Span, id: usize) -> Stmt {
    let (c, i) = (format!("$for{id}"), format!("$fori{id}"));
    let e = |kind: ExprKind| Expr { kind, span };
    let var = |n: &str| Expr { kind: ExprKind::Var(n.to_string()), span };
    let bind = |n: &str, m: bool| Pattern { kind: PatKind::Bind(n.to_string(), m), span };
    let brk = || Block { stmts: vec![Stmt::Break { label: None, value: None, span }], tail: None, span };
    let call = |recv: Expr, m: &str| Expr { kind: ExprKind::MethodCall(Box::new(recv), m.into(), vec![]), span };
    let (setup, lp) = match iter.kind {
        ExprKind::Ref(false, list) => {
            let usize_ty = TypeExpr { name: "usize".into(), args: vec![], span };
            let setup = vec![
                Stmt::Let {
                    pat: bind(&c, false),
                    ty: None,
                    value: e(ExprKind::Ref(false, list)),
                    else_block: None,
                    span,
                },
                Stmt::Let {
                    pat: bind(&i, true),
                    ty: Some(usize_ty),
                    value: e(ExprKind::Int(0)),
                    else_block: None,
                    span,
                },
            ];
            let done = e(ExprKind::Binary(BinOp::Ge, Box::new(var(&i)), Box::new(call(var(&c), "len"))));
            let elem = e(ExprKind::Index(Box::new(var(&c)), Box::new(var(&i))));
            let mut stmts = vec![
                Stmt::Expr(e(ExprKind::If(Box::new(done), brk(), None))),
                Stmt::Let { pat, ty: None, value: e(ExprKind::Ref(false, Box::new(elem))), else_block: None, span },
                Stmt::Assign { target: var(&i), op: Some(BinOp::Add), value: e(ExprKind::Int(1)), span },
            ];
            stmts.push(Stmt::Expr(e(ExprKind::Block(body))));
            (setup, Block { stmts, tail: None, span })
        }
        _ => {
            let setup = vec![
                Stmt::Let { pat: bind(&c, true), ty: None, value: iter, else_block: None, span },
                Stmt::Expr(call(var(&c), "reverse")),
            ];
            let arms = vec![
                Arm {
                    pat: Pattern { kind: PatKind::Variant("Some".into(), vec![pat]), span },
                    guard: None,
                    body: e(ExprKind::Block(body)),
                    span,
                },
                Arm { pat: Pattern { kind: PatKind::Wild, span }, guard: None, body: e(ExprKind::Block(brk())), span },
            ];
            let m = e(ExprKind::Match(Box::new(call(var(&c), "pop")), arms));
            (setup, Block { stmts: vec![Stmt::Expr(m)], tail: None, span })
        }
    };
    let mut stmts = setup;
    stmts.push(Stmt::Expr(e(ExprKind::Loop(lp, label))));
    Stmt::Expr(e(ExprKind::Block(Block { stmts, tail: None, span })))
}

fn is_upper(name: &str) -> bool {
    name.rsplit("::").next().and_then(|s| s.chars().next()).is_some_and(|c| c.is_ascii_uppercase())
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

    /// Run `f` one nesting level deeper, failing cleanly past [`MAX_DEPTH`].
    fn deep<X>(&mut self, f: impl FnOnce(&mut Self) -> PResult<X>) -> PResult<X> {
        self.check_depth(1)?;
        self.depth += 1;
        let r = f(self);
        self.depth -= 1;
        r
    }

    /// Fail if `extra` more levels (from a chain like `a + b + c` or `x.f().g()`) would be too deep.
    fn check_depth(&self, extra: usize) -> PResult<()> {
        if self.depth + extra <= MAX_DEPTH {
            return Ok(());
        }
        Err(ParseError {
            message: format!(
                "this nests too deeply (more than {MAX_DEPTH} levels); split it up with `let` or helper functions"
            ),
            span: self.peek().span,
        })
    }

    fn is_depth_error(e: &ParseError) -> bool {
        e.message.starts_with("this nests too deeply")
    }

    /// After an error in a top-level item, skip to the next item that starts a line.
    fn sync_item(&mut self, start: usize) {
        if self.pos == start {
            self.bump();
        }
        while !self.at(&T::Eof) {
            let item = matches!(self.peek().kind, T::Fn | T::Struct | T::Enum | T::Impl | T::Const | T::Trait);
            if item && self.col0.get(self.pos).copied().unwrap_or(false) {
                return;
            }
            self.bump();
        }
    }

    /// After an error in a statement, skip to the end of it: a `;`, the block's
    /// closing `}`, or the next line at the same bracket depth.
    fn sync_stmt(&mut self, start: usize) {
        if self.pos == start && !matches!(self.peek().kind, T::RBrace | T::Eof) {
            self.bump();
        }
        let mut depth = 0usize;
        loop {
            match self.peek().kind {
                T::Eof => return,
                T::RBrace if depth == 0 => return,
                T::Semi if depth == 0 => {
                    self.bump();
                    return;
                }
                T::LBrace | T::LParen | T::LBracket => depth += 1,
                T::RBrace | T::RParen | T::RBracket => depth = depth.saturating_sub(1),
                _ if depth == 0 && self.pos != start && self.first_on_line.get(self.pos).copied().unwrap_or(false) => {
                    return
                }
                _ => {}
            }
            self.bump();
        }
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
        self.deep(|p| p.ty_inner())
    }

    fn ty_inner(&mut self) -> PResult<TypeExpr> {
        let start = self.peek().span;
        if self.eat(&T::LParen) {
            // `()`, `(T)`, `(T,)`, `(A, B)`
            let mut args = Vec::new();
            let mut trailing_comma = false;
            while !self.at(&T::RParen) {
                args.push(self.ty()?);
                trailing_comma = self.eat(&T::Comma);
                if !trailing_comma {
                    break;
                }
            }
            let end = self.expect(T::RParen)?;
            if args.len() == 1 && !trailing_comma {
                return Ok(args.pop().unwrap());
            }
            return Ok(TypeExpr { name: TypeExpr::TUPLE.into(), args, span: start.to(end) });
        }
        let (name, span) = self.ident()?;
        let mut args = Vec::new();
        if self.eat(&T::LBracket) {
            while !self.at(&T::RBracket) {
                args.push(self.ty()?);
                if !self.eat(&T::Comma) {
                    break;
                }
            }
            self.expect(T::RBracket)?;
        }
        Ok(TypeExpr { name, args, span: span.to(self.prev_span()) })
    }

    /// Optional generic parameter list without bounds: `[T, U]`.
    fn generics(&mut self) -> PResult<Vec<String>> {
        let (names, bounds) = self.bounded_generics()?;
        if let Some(b) = bounds.first() {
            return Err(ParseError {
                message: "put trait bounds on the `impl` or `fn` that needs them, not on the type".into(),
                span: b.span,
            });
        }
        Ok(names)
    }

    /// Optional generic parameter list with bounds: `[T: Show + Eq, U]`.
    fn bounded_generics(&mut self) -> PResult<(Vec<String>, Vec<Bound>)> {
        let mut names = Vec::new();
        let mut bounds = Vec::new();
        if self.eat(&T::LBracket) {
            while !self.at(&T::RBracket) {
                let (name, _) = self.ident()?;
                if self.eat(&T::Colon) {
                    self.bound_list(&name, &mut bounds)?;
                }
                names.push(name);
                if !self.eat(&T::Comma) {
                    break;
                }
            }
            self.expect(T::RBracket)?;
        }
        Ok((names, bounds))
    }

    /// `A + B + C` after `T:`.
    fn bound_list(&mut self, param: &str, out: &mut Vec<Bound>) -> PResult<()> {
        loop {
            let (trait_name, span) = self.ident()?;
            out.push(Bound { param: param.to_string(), trait_name, span });
            if !self.eat(&T::Plus) {
                return Ok(());
            }
        }
    }

    /// Optional `uses io, panic` effect list after the signature. `uses` is only a
    /// keyword in this position, so it stays usable as a name elsewhere.
    fn uses_clause(&mut self) -> PResult<Option<Vec<(String, Span)>>> {
        if !matches!(&self.peek().kind, T::Ident(n) if n == "uses") {
            return Ok(None);
        }
        self.bump();
        let mut effects = Vec::new();
        while let T::Ident(_) = self.peek().kind {
            effects.push(self.ident()?);
            if !self.eat(&T::Comma) {
                break;
            }
        }
        Ok(Some(effects))
    }

    /// Optional `where T: A + B, U: C` before a body.
    fn where_clause(&mut self, out: &mut Vec<Bound>) -> PResult<()> {
        if self.eat(&T::Where) {
            while !self.at(&T::LBrace) {
                let (name, _) = self.ident()?;
                self.expect(T::Colon)?;
                self.bound_list(&name, out)?;
                if !self.eat(&T::Comma) {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Run `f` with struct literals allowed again (inside parentheses, blocks, arguments).
    fn nested<X>(&mut self, f: impl FnOnce(&mut Self) -> PResult<X>) -> PResult<X> {
        let saved = std::mem::replace(&mut self.no_struct, false);
        let r = f(self);
        self.no_struct = saved;
        r
    }

    /// Parse an `if`/`while`/`match` header expression, where struct literals are not allowed.
    fn header_expr(&mut self) -> PResult<Expr> {
        let saved = std::mem::replace(&mut self.no_struct, true);
        let r = self.expr();
        self.no_struct = saved;
        r
    }

    // ---------- items ----------

    fn function(&mut self) -> PResult<Function> {
        self.function_in(false)
    }

    /// Parse a function. In a trait, the body may be left out.
    fn function_in(&mut self, in_trait: bool) -> PResult<Function> {
        let start = self.expect(T::Fn)?;
        let (name, _) = self.ident()?;
        let (generics, mut bounds) = self.bounded_generics()?;
        self.expect(T::LParen)?;
        let mut params = Vec::new();
        while !self.at(&T::RParen) {
            params.push(self.param()?);
            if !self.eat(&T::Comma) {
                break;
            }
        }
        self.expect(T::RParen)?;
        let ret = if self.eat(&T::Arrow) { Some(self.ty()?) } else { None };
        let uses = self.uses_clause()?;
        self.where_clause(&mut bounds)?;
        if in_trait && !self.at(&T::LBrace) {
            self.eat(&T::Semi);
            let span = start.to(self.prev_span());
            let body = Block { stmts: vec![], tail: None, span };
            return Ok(Function { name, generics, bounds, params, ret, body, has_body: false, uses, span });
        }
        let body = self.block()?;
        Ok(Function { name, generics, bounds, params, ret, span: start.to(body.span), body, has_body: true, uses })
    }

    fn param(&mut self) -> PResult<Param> {
        let start = self.peek().span;
        // `self`, `mut self`, `&self`, `&mut self`
        let is_self = |p: &Self, off: usize| matches!(p.peek_at(off), T::Ident(n) if n == "self");
        let self_form = if is_self(self, 0) {
            Some((0, ParamMode::Value, false))
        } else if self.at(&T::Mut) && is_self(self, 1) {
            Some((1, ParamMode::Value, true))
        } else if self.at(&T::Amp) && is_self(self, 1) {
            Some((1, ParamMode::Ref, false))
        } else if self.at(&T::Amp) && self.peek_at(1) == &T::Mut && is_self(self, 2) {
            Some((2, ParamMode::RefMut, false))
        } else {
            None
        };
        if let Some((skip, mode, mutable)) = self_form {
            for _ in 0..=skip {
                self.bump();
            }
            return Ok(Param {
                name: "self".into(),
                ty: None,
                mode,
                mutable,
                default: None,
                span: start.to(self.prev_span()),
            });
        }
        let mutable = self.eat(&T::Mut);
        let (name, _) = self.ident()?;
        self.expect(T::Colon)?;
        let mode = if self.eat(&T::Amp) {
            if self.eat(&T::Mut) {
                ParamMode::RefMut
            } else {
                ParamMode::Ref
            }
        } else {
            ParamMode::Value
        };
        let ty = self.ty()?;
        let default = if self.eat(&T::Eq) { Some(self.expr()?) } else { None };
        let end = default.as_ref().map_or(ty.span, |d| d.span);
        Ok(Param { name, ty: Some(ty), mode, mutable, default, span: start.to(end) })
    }

    fn struct_def(&mut self) -> PResult<StructDef> {
        let start = self.expect(T::Struct)?;
        let (name, _) = self.ident()?;
        let generics = self.generics()?;
        self.expect(T::LBrace)?;
        let mut fields = Vec::new();
        let mut defaults = Vec::new();
        while !self.at(&T::RBrace) {
            let (f, _) = self.ident()?;
            self.expect(T::Colon)?;
            fields.push((f, self.ty()?));
            defaults.push(if self.eat(&T::Eq) { Some(self.expr()?) } else { None });
            if !self.eat(&T::Comma) {
                break;
            }
        }
        let end = self.expect(T::RBrace)?;
        Ok(StructDef { name, generics, fields, defaults, span: start.to(end) })
    }

    fn enum_def(&mut self) -> PResult<EnumDef> {
        let start = self.expect(T::Enum)?;
        let (name, _) = self.ident()?;
        let generics = self.generics()?;
        self.expect(T::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&T::RBrace) {
            let (vname, vspan) = self.ident()?;
            let mut fields = Vec::new();
            if self.eat(&T::LParen) {
                while !self.at(&T::RParen) {
                    fields.push(self.ty()?);
                    if !self.eat(&T::Comma) {
                        break;
                    }
                }
                self.expect(T::RParen)?;
            }
            variants.push(Variant { name: vname, fields, span: vspan.to(self.prev_span()) });
            if !self.eat(&T::Comma) {
                break;
            }
        }
        let end = self.expect(T::RBrace)?;
        Ok(EnumDef { name, generics, variants, span: start.to(end) })
    }

    fn impl_block(&mut self) -> PResult<ImplBlock> {
        let start = self.expect(T::Impl)?;
        let (generics, mut bounds) = self.bounded_generics()?;
        let mut ty = self.ty()?;
        let mut trait_name = None;
        if self.eat(&T::For) {
            if !ty.args.is_empty() || ty.name == TypeExpr::TUPLE {
                return Err(ParseError { message: "expected a trait name before `for`".into(), span: ty.span });
            }
            trait_name = Some((ty.name.clone(), ty.span));
            ty = self.ty()?;
        }
        self.where_clause(&mut bounds)?;
        self.expect(T::LBrace)?;
        let mut methods = Vec::new();
        while self.at(&T::Fn) {
            methods.push(self.function()?);
        }
        let end = self.expect(T::RBrace)?;
        Ok(ImplBlock { generics, bounds, trait_name, ty, methods, span: start.to(end) })
    }

    fn trait_def(&mut self) -> PResult<TraitDef> {
        let start = self.expect(T::Trait)?;
        let (name, _) = self.ident()?;
        self.expect(T::LBrace)?;
        let mut methods = Vec::new();
        while self.at(&T::Fn) {
            methods.push(self.function_in(true)?);
        }
        let end = self.expect(T::RBrace)?;
        Ok(TraitDef { name, methods, span: start.to(end) })
    }

    fn const_def(&mut self) -> PResult<ConstDef> {
        let start = self.expect(T::Const)?;
        let (name, _) = self.ident()?;
        self.expect(T::Colon)?;
        let ty = self.ty()?;
        self.expect(T::Eq)?;
        let value = self.expr()?;
        self.eat(&T::Semi);
        Ok(ConstDef { name, ty, span: start.to(value.span), value })
    }

    // ---------- statements ----------

    fn block(&mut self) -> PResult<Block> {
        self.deep(|p| p.nested(|p| p.block_inner()))
    }

    fn block_inner(&mut self) -> PResult<Block> {
        let start = self.expect(T::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail = None;
        while !self.at(&T::RBrace) {
            if self.at(&T::Eof) {
                return self.error("`}`");
            }
            let stmt_start = self.pos;
            let stmt = match self.stmt() {
                Ok(s) => s,
                // Record the error and carry on with the next statement.
                Err(e)
                    if !Self::is_depth_error(&e)
                        && self.errors.len() < MAX_ERRORS
                        && !self.first_on_line.is_empty() =>
                {
                    self.errors.push(e);
                    self.sync_stmt(stmt_start);
                    continue;
                }
                Err(e) => return Err(e),
            };
            let had_semi = self.eat(&T::Semi);
            match stmt {
                Stmt::Expr(e) if !had_semi && self.at(&T::RBrace) => tail = Some(Box::new(e)),
                s => stmts.push(s),
            }
        }
        let end = self.expect(T::RBrace)?;
        Ok(Block { stmts, tail, span: start.to(end) })
    }

    /// True if the next token cannot start an expression (used after `return` and `break`).
    fn at_expr_end(&self) -> bool {
        matches!(self.peek().kind, T::Semi | T::RBrace | T::Comma | T::Eof)
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.peek().span;
        match self.peek().kind {
            T::Let => {
                self.bump();
                let pat = self.pattern()?;
                let ty = if self.eat(&T::Colon) { Some(self.ty()?) } else { None };
                self.expect(T::Eq)?;
                let value = self.expr()?;
                let else_block = if self.eat(&T::Else) { Some(self.block()?) } else { None };
                let end = else_block.as_ref().map_or(value.span, |b| b.span);
                Ok(Stmt::Let { pat, ty, span: start.to(end), value, else_block })
            }
            T::While if self.peek_at(1) == &T::Let => {
                // `while let P = e { body }` is `loop { match e { P => body, _ => break } }`.
                self.bump();
                self.bump();
                let label = self.label.take();
                let pat = self.pattern()?;
                self.expect(T::Eq)?;
                let scrut = self.header_expr()?;
                let body = self.block()?;
                let span = start.to(body.span);
                let brk = Block { stmts: vec![Stmt::Break { label: None, value: None, span }], tail: None, span };
                let arms = vec![
                    Arm {
                        span: pat.span,
                        pat,
                        guard: None,
                        body: Expr { span: body.span, kind: ExprKind::Block(body) },
                    },
                    Arm {
                        pat: Pattern { kind: PatKind::Wild, span },
                        guard: None,
                        span,
                        body: Expr { kind: ExprKind::Block(brk), span },
                    },
                ];
                let m = Expr { kind: ExprKind::Match(Box::new(scrut), arms), span };
                let lp = Block { stmts: vec![Stmt::Expr(m)], tail: None, span };
                Ok(Stmt::Expr(Expr { kind: ExprKind::Loop(lp, label), span }))
            }
            T::While => {
                self.bump();
                let label = self.label.take();
                let cond = self.header_expr()?;
                let body = self.block()?;
                Ok(Stmt::While { cond, label, span: start.to(body.span), body })
            }
            T::Label(_) if self.peek_at(1) == &T::Colon => {
                let T::Label(name) = self.bump().kind else { unreachable!() };
                self.bump();
                if !matches!(self.peek().kind, T::While | T::For | T::Loop) {
                    return self.error("`loop`, `while` or `for` after a label");
                }
                self.label = Some(name);
                self.stmt()
            }
            T::For => {
                self.bump();
                let label = self.label.take();
                let pat = self.pattern()?;
                self.expect(T::In)?;
                let range_start = self.header_expr()?;
                let inclusive = if self.eat(&T::DotDotEq) {
                    true
                } else if self.eat(&T::DotDot) {
                    false
                } else {
                    let body = self.block()?;
                    return self.for_each(pat, range_start, body, label, start);
                };
                let var = match pat.kind {
                    PatKind::Bind(name, false) => name,
                    PatKind::Wild => "_".to_string(),
                    _ => {
                        return Err(ParseError {
                            message: "a range loop binds one name, like `for i in 0..n`".into(),
                            span: pat.span,
                        })
                    }
                };
                let end = self.header_expr()?;
                let body = self.block()?;
                Ok(Stmt::For { var, start: range_start, end, inclusive, label, span: start.to(body.span), body })
            }
            T::Return | T::Break => {
                let is_return = self.bump().kind == T::Return;
                let label = match self.peek().kind.clone() {
                    T::Label(l) if !is_return => {
                        self.bump();
                        Some(l)
                    }
                    _ => None,
                };
                let value = if self.at_expr_end() { None } else { Some(self.expr()?) };
                let span = start.to(self.prev_span());
                Ok(if is_return { Stmt::Return { value, span } } else { Stmt::Break { label, value, span } })
            }
            T::Continue => {
                self.bump();
                let label = match self.peek().kind.clone() {
                    T::Label(l) => {
                        self.bump();
                        Some(l)
                    }
                    _ => None,
                };
                Ok(Stmt::Continue { label, span: start.to(self.prev_span()) })
            }
            T::Defer => {
                self.bump();
                let body = self.expr()?;
                Ok(Stmt::Defer { span: start.to(body.span), body })
            }
            // Block-like expressions end the statement at their closing brace, as in Rust.
            T::If | T::Match | T::Loop | T::LBrace => Ok(Stmt::Expr(self.primary()?)),
            _ => {
                let target = self.expr()?;
                let op = match self.peek().kind {
                    T::Eq => None,
                    T::PlusEq => Some(BinOp::Add),
                    T::MinusEq => Some(BinOp::Sub),
                    T::StarEq => Some(BinOp::Mul),
                    T::SlashEq => Some(BinOp::Div),
                    T::PercentEq => Some(BinOp::Rem),
                    T::AmpEq => Some(BinOp::BitAnd),
                    T::PipeEq => Some(BinOp::BitOr),
                    T::CaretEq => Some(BinOp::BitXor),
                    T::ShlEq => Some(BinOp::Shl),
                    T::ShrEq => Some(BinOp::Shr),
                    _ => return Ok(Stmt::Expr(target)),
                };
                self.bump();
                let value = self.expr()?;
                Ok(Stmt::Assign { target, op, span: start.to(value.span), value })
            }
        }
    }

    // ---------- expressions ----------

    fn expr(&mut self) -> PResult<Expr> {
        self.deep(|p| p.binary(0))
    }

    /// Precedence climbing. Higher numbers bind tighter.
    fn binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.unary()?;
        // Each operator in a chain like `a + b + c` nests the tree one level deeper.
        let mut chain = 0;
        loop {
            chain += 1;
            self.check_depth(chain)?;
            if self.at(&T::PipeGt) {
                if min_prec > 0 {
                    return Ok(lhs);
                }
                self.bump();
                let rhs = self.binary(1)?;
                lhs = pipe(lhs, rhs)?;
                continue;
            }
            if matches!(self.peek().kind, T::Star | T::Minus | T::Amp) && self.line_start() {
                return Ok(lhs);
            }
            let (op, prec) = match self.peek().kind {
                T::OrOr => (BinOp::Or, 1),
                T::AndAnd => (BinOp::And, 2),
                T::EqEq => (BinOp::Eq, 3),
                T::NotEq => (BinOp::Ne, 3),
                T::Lt => (BinOp::Lt, 3),
                T::LtEq => (BinOp::Le, 3),
                T::Gt => (BinOp::Gt, 3),
                T::GtEq => (BinOp::Ge, 3),
                // Same order as Rust: `|` < `^` < `&` < shifts < `+ -` < `* / %`.
                T::Pipe => (BinOp::BitOr, 4),
                T::Caret => (BinOp::BitXor, 5),
                T::Amp => (BinOp::BitAnd, 6),
                T::Shl => (BinOp::Shl, 7),
                T::Shr => (BinOp::Shr, 7),
                T::Plus => (BinOp::Add, 8),
                T::Minus => (BinOp::Sub, 8),
                T::Star => (BinOp::Mul, 9),
                T::Slash => (BinOp::Div, 9),
                T::Percent => (BinOp::Rem, 9),
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

    /// A prefix expression followed by any `as` casts. As in Rust, prefix operators
    /// bind tighter than `as`: `-1 as u32` is `(-1) as u32`.
    fn unary(&mut self) -> PResult<Expr> {
        let mut e = self.prefix()?;
        let mut chain = 0;
        while self.eat(&T::As) {
            chain += 1;
            self.check_depth(chain)?;
            let ty = self.ty()?;
            let span = e.span.to(ty.span);
            e = Expr { kind: ExprKind::Cast(Box::new(e), ty), span };
        }
        Ok(e)
    }

    fn prefix(&mut self) -> PResult<Expr> {
        self.deep(|p| p.prefix_inner())
    }

    fn prefix_inner(&mut self) -> PResult<Expr> {
        let start = self.peek().span;
        let kind = match self.peek().kind {
            T::Minus | T::Bang | T::Star => {
                let op = match self.bump().kind {
                    T::Minus => UnOp::Neg,
                    T::Bang => UnOp::Not,
                    _ => UnOp::Deref,
                };
                let inner = self.prefix()?;
                ExprKind::Unary(op, Box::new(inner))
            }
            T::Amp => {
                self.bump();
                let mutable = self.eat(&T::Mut);
                let inner = self.prefix()?;
                ExprKind::Ref(mutable, Box::new(inner))
            }
            _ => return self.postfix(),
        };
        Ok(Expr { kind, span: start.to(self.prev_span()) })
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        let mut chain = 0;
        loop {
            chain += 1;
            self.check_depth(chain)?;
            if self.at(&T::Question) {
                let end = self.bump().span;
                let span = e.span.to(end);
                e = Expr { kind: ExprKind::Try(Box::new(e)), span };
                continue;
            }
            if self.at(&T::LBracket) && !self.line_start() {
                self.bump();
                let index = self.nested(|p| p.expr())?;
                let end = self.expect(T::RBracket)?;
                let span = e.span.to(end);
                e = Expr { kind: ExprKind::Index(Box::new(e), Box::new(index)), span };
                continue;
            }
            if !self.eat(&T::Dot) {
                break;
            }
            // Tuple fields: `t.0`. The lexer reads `t.0.1` as `t`, `.`, `0.1`.
            match self.peek().kind.clone() {
                T::Int(n, None) => {
                    let span = e.span.to(self.bump().span);
                    e = Expr { kind: ExprKind::Field(Box::new(e), n.to_string()), span };
                    continue;
                }
                T::Float(_, None) => {
                    let tok = self.bump();
                    let span = e.span.to(tok.span);
                    // Recover both indexes from the token's text length is not possible; use the value.
                    let text = format!(
                        "{}",
                        match tok.kind {
                            T::Float(x, _) => x,
                            _ => unreachable!(),
                        }
                    );
                    for part in text.split('.') {
                        e = Expr { kind: ExprKind::Field(Box::new(e), part.to_string()), span };
                    }
                    continue;
                }
                _ => {}
            }
            let (name, nspan) = self.ident()?;
            if self.at(&T::LParen) {
                let args = self.args()?;
                let span = e.span.to(self.prev_span());
                e = Expr { kind: ExprKind::MethodCall(Box::new(e), name, args), span };
            } else {
                let span = e.span.to(nspan);
                e = Expr { kind: ExprKind::Field(Box::new(e), name), span };
            }
        }
        Ok(e)
    }

    fn args(&mut self) -> PResult<Vec<Arg>> {
        self.expect(T::LParen)?;
        self.nested(|p| {
            let mut args = Vec::new();
            while !p.at(&T::RParen) {
                args.push(p.arg()?);
                if !p.eat(&T::Comma) {
                    break;
                }
            }
            p.expect(T::RParen)?;
            Ok(args)
        })
    }

    fn arg(&mut self) -> PResult<Arg> {
        let start = self.peek().span;
        let name = match (self.peek().kind.clone(), self.peek_at(1)) {
            (T::Ident(n), T::Colon) => {
                self.bump();
                self.bump();
                Some(n)
            }
            _ => None,
        };
        let value = self.expr()?;
        Ok(Arg { name, span: start.to(value.span), value })
    }

    /// `a::b::c` (the first identifier has already been read).
    fn path_rest(&mut self, mut name: String) -> PResult<String> {
        while self.at(&T::ColonColon) {
            self.bump();
            let (seg, _) = self.ident()?;
            name.push_str("::");
            name.push_str(&seg);
        }
        Ok(name)
    }

    fn primary(&mut self) -> PResult<Expr> {
        let tok = self.peek().clone();
        let span = tok.span;
        let kind = match tok.kind {
            T::Int(n, suffix) => {
                self.bump();
                return Ok(with_suffix(ExprKind::Int(n as i128), suffix, span));
            }
            T::Float(x, suffix) => {
                self.bump();
                return Ok(with_suffix(ExprKind::Float(x), suffix, span));
            }
            T::Char(c) => {
                self.bump();
                ExprKind::Char(c)
            }
            T::LBracket => {
                self.bump();
                let mut items = Vec::new();
                let kind = self.nested(|p| {
                    while !p.at(&T::RBracket) {
                        items.push(p.expr()?);
                        if items.len() == 1 && p.eat(&T::Semi) {
                            let count = p.expr()?;
                            return Ok(ExprKind::ListRepeat(Box::new(items.pop().unwrap()), Box::new(count)));
                        }
                        if !p.eat(&T::Comma) {
                            break;
                        }
                    }
                    Ok(ExprKind::List(std::mem::take(&mut items)))
                })?;
                let end = self.expect(T::RBracket)?;
                return Ok(Expr { kind, span: span.to(end) });
            }
            T::Str(s) => {
                self.bump();
                ExprKind::Str(s)
            }
            T::InterpStr(parts) => {
                self.bump();
                ExprKind::Interp(parts.into_iter().map(|x| interp_part(x, self.depth)).collect::<PResult<_>>()?)
            }
            T::True | T::False => {
                self.bump();
                ExprKind::Bool(tok.kind == T::True)
            }
            T::Ident(name) => {
                self.bump();
                let path = self.path_rest(name)?;
                if self.at(&T::Bang) && matches!(self.peek_at(1), T::LParen | T::LBracket | T::LBrace) {
                    // Rust habit: `println!(...)`, `vec![...]`.
                    return Err(ParseError {
                        message: format!("Om has no macros; call `{path}(...)` as a function, without `!`"),
                        span: self.peek().span,
                    });
                }
                // As with `[`, a `(` that starts a line begins a new expression
                // instead of calling the name on the line before.
                if self.at(&T::LParen) && !self.line_start() {
                    let args = self.args()?;
                    return Ok(Expr { kind: ExprKind::Call(path, args), span: span.to(self.prev_span()) });
                }
                if self.at(&T::LBrace) && !self.no_struct && is_upper(&path) {
                    return self.struct_lit(path, span);
                }
                ExprKind::Var(path)
            }
            T::LParen => {
                self.bump();
                let (mut items, trailing) = self.nested(|p| {
                    let mut items = Vec::new();
                    let mut trailing = false;
                    while !p.at(&T::RParen) {
                        items.push(p.expr()?);
                        trailing = p.eat(&T::Comma);
                        if !trailing {
                            break;
                        }
                    }
                    Ok((items, trailing))
                })?;
                let end = self.expect(T::RParen)?;
                if items.len() == 1 && !trailing {
                    let mut inner = items.pop().unwrap();
                    inner.span = span.to(end);
                    return Ok(inner);
                }
                return Ok(Expr { kind: ExprKind::Tuple(items), span: span.to(end) });
            }
            T::If => return self.if_expr(),
            T::Match => return self.match_expr(),
            T::Loop => {
                self.bump();
                let label = self.label.take();
                let body = self.block()?;
                return Ok(Expr { span: span.to(body.span), kind: ExprKind::Loop(body, label) });
            }
            T::Label(_) if self.peek_at(1) == &T::Colon && self.peek_at(2) == &T::Loop => {
                let T::Label(name) = self.bump().kind else { unreachable!() };
                self.bump();
                self.label = Some(name);
                let mut e = self.primary()?;
                e.span = span.to(e.span);
                return Ok(e);
            }
            T::LBrace => {
                let b = self.block()?;
                return Ok(Expr { span: b.span, kind: ExprKind::Block(b) });
            }
            _ => return self.error("an expression"),
        };
        Ok(Expr { kind, span })
    }

    fn struct_lit(&mut self, name: String, start: Span) -> PResult<Expr> {
        self.expect(T::LBrace)?;
        let fields = self.nested(|p| {
            let mut fields = Vec::new();
            while !p.at(&T::RBrace) {
                let (f, fspan) = p.ident()?;
                // `Point { x, y }` is shorthand for `Point { x: x, y: y }`.
                let value =
                    if p.eat(&T::Colon) { p.expr()? } else { Expr { kind: ExprKind::Var(f.clone()), span: fspan } };
                fields.push((f, value));
                if !p.eat(&T::Comma) {
                    break;
                }
            }
            Ok(fields)
        })?;
        let end = self.expect(T::RBrace)?;
        Ok(Expr { kind: ExprKind::StructLit(name, fields), span: start.to(end) })
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        self.deep(|p| p.if_inner())
    }

    fn if_inner(&mut self) -> PResult<Expr> {
        let start = self.expect(T::If)?;
        if self.eat(&T::Let) {
            // `if let P = e { a } else { b }` is `match e { P => { a }, _ => { b } }`.
            let pat = self.pattern()?;
            self.expect(T::Eq)?;
            let scrut = self.header_expr()?;
            let then = self.block()?;
            let els = if self.eat(&T::Else) {
                if self.at(&T::If) {
                    let nested = self.if_expr()?;
                    Block { span: nested.span, stmts: vec![], tail: Some(Box::new(nested)) }
                } else {
                    self.block()?
                }
            } else {
                Block { stmts: vec![], tail: None, span: then.span }
            };
            let span = start.to(els.span);
            let arms = vec![
                Arm { span: pat.span, pat, guard: None, body: Expr { span: then.span, kind: ExprKind::Block(then) } },
                Arm {
                    pat: Pattern { kind: PatKind::Wild, span: els.span },
                    guard: None,
                    span: els.span,
                    body: Expr { span: els.span, kind: ExprKind::Block(els) },
                },
            ];
            return Ok(Expr { kind: ExprKind::Match(Box::new(scrut), arms), span });
        }
        let cond = self.header_expr()?;
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

    fn match_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(T::Match)?;
        let scrutinee = self.header_expr()?;
        self.expect(T::LBrace)?;
        let arms = self.nested(|p| {
            let mut arms = Vec::new();
            while !p.at(&T::RBrace) {
                let pat = p.pattern()?;
                let guard = if p.eat(&T::If) { Some(p.expr()?) } else { None };
                p.expect(T::FatArrow)?;
                let body = match p.peek().kind {
                    // Statements that make sense as an arm body: `None => return 0,`
                    T::Return | T::Break | T::Continue => {
                        let bstart = p.peek().span;
                        let s = p.stmt()?;
                        let span = bstart.to(p.prev_span());
                        Expr { kind: ExprKind::Block(Block { stmts: vec![s], tail: None, span }), span }
                    }
                    _ => p.expr()?,
                };
                let block_like = matches!(body.kind, ExprKind::Block(_) | ExprKind::If(..) | ExprKind::Match(..));
                arms.push(Arm { span: pat.span.to(body.span), pat, guard, body });
                if !p.eat(&T::Comma) && !block_like && !p.at(&T::RBrace) {
                    return p.error("`,` between match arms");
                }
            }
            Ok(arms)
        })?;
        let end = self.expect(T::RBrace)?;
        Ok(Expr { kind: ExprKind::Match(Box::new(scrutinee), arms), span: start.to(end) })
    }

    fn pattern(&mut self) -> PResult<Pattern> {
        self.deep(|p| p.pattern_inner())
    }

    fn pattern_inner(&mut self) -> PResult<Pattern> {
        self.eat(&T::Pipe); // allow a leading `|`
        let first = self.pattern_alt()?;
        if !self.at(&T::Pipe) {
            return Ok(first);
        }
        let mut alts = vec![first];
        while self.eat(&T::Pipe) {
            alts.push(self.pattern_alt()?);
        }
        let span = alts[0].span.to(alts.last().unwrap().span);
        Ok(Pattern { kind: PatKind::Or(alts), span })
    }

    fn int_lit(&mut self) -> PResult<i128> {
        let neg = self.eat(&T::Minus);
        match self.peek().kind {
            T::Int(n, _) => {
                self.bump();
                Ok(if neg { -(n as i128) } else { n as i128 })
            }
            _ => self.error("an integer"),
        }
    }

    fn char_lit(&mut self) -> PResult<char> {
        match self.peek().kind {
            T::Char(c) => {
                self.bump();
                Ok(c)
            }
            _ => self.error("a character"),
        }
    }

    fn pattern_alt(&mut self) -> PResult<Pattern> {
        let start = self.peek().span;
        let kind = match self.peek().kind.clone() {
            T::Char(_) => {
                let lo = self.char_lit()?;
                if self.eat(&T::DotDotEq) {
                    PatKind::CharRange(lo, self.char_lit()?, true)
                } else if self.eat(&T::DotDot) {
                    PatKind::CharRange(lo, self.char_lit()?, false)
                } else {
                    PatKind::Char(lo)
                }
            }
            T::Int(..) | T::Minus => {
                let lo = self.int_lit()?;
                if self.eat(&T::DotDotEq) {
                    PatKind::Range(lo, self.int_lit()?, true)
                } else if self.eat(&T::DotDot) {
                    PatKind::Range(lo, self.int_lit()?, false)
                } else {
                    PatKind::Int(lo)
                }
            }
            T::True | T::False => PatKind::Bool(self.bump().kind == T::True),
            T::Mut => {
                self.bump();
                PatKind::Bind(self.ident()?.0, true)
            }
            T::LParen => {
                self.bump();
                let mut subs = Vec::new();
                while !self.at(&T::RParen) {
                    subs.push(self.pattern()?);
                    if !self.eat(&T::Comma) {
                        break;
                    }
                }
                self.expect(T::RParen)?;
                PatKind::Tuple(subs)
            }
            T::Ident(name) if name == "_" => {
                self.bump();
                PatKind::Wild
            }
            T::Ident(name) => {
                self.bump();
                let path = self.path_rest(name)?;
                if self.eat(&T::LParen) {
                    let mut subs = Vec::new();
                    while !self.at(&T::RParen) {
                        subs.push(self.pattern()?);
                        if !self.eat(&T::Comma) {
                            break;
                        }
                    }
                    self.expect(T::RParen)?;
                    PatKind::Variant(path, subs)
                } else if self.at(&T::LBrace) && (path.contains("::") || is_upper(&path)) {
                    // Struct pattern: `Point { x, y: pat, .. }`
                    self.bump(); // eat `{`
                    let mut fields = Vec::new();
                    let mut rest = false;
                    while !self.at(&T::RBrace) {
                        if self.eat(&T::DotDot) {
                            rest = true;
                            self.eat(&T::Comma);
                            break;
                        }
                        let (fname, _) = self.ident()?;
                        let pat = if self.eat(&T::Colon) {
                            self.pattern()?
                        } else {
                            // Shorthand: `x` means `x: x`
                            Pattern { kind: PatKind::Bind(fname.clone(), false), span: self.prev_span() }
                        };
                        fields.push((fname, pat));
                        if !self.eat(&T::Comma) {
                            break;
                        }
                    }
                    self.expect(T::RBrace)?;
                    PatKind::Struct(path, fields, rest)
                } else if path.contains("::") || is_upper(&path) {
                    PatKind::Variant(path, vec![])
                } else {
                    PatKind::Bind(path, false)
                }
            }
            _ => return self.error("a pattern"),
        };
        Ok(Pattern { kind, span: start.to(self.prev_span()) })
    }
}

/// `5u8` becomes `5 as u8`; the type checker treats a cast literal as a literal of that type.
fn with_suffix(kind: ExprKind, suffix: Option<String>, span: Span) -> Expr {
    let e = Expr { kind, span };
    match suffix {
        None => e,
        Some(name) => {
            let ty = TypeExpr { name, args: vec![], span };
            Expr { kind: ExprKind::Cast(Box::new(e), ty), span }
        }
    }
}

/// Desugar `lhs |> f(a, b)` into `f(lhs, a, b)`, and `lhs |> f` into `f(lhs)`.
fn pipe(lhs: Expr, rhs: Expr) -> PResult<Expr> {
    let span = lhs.span.to(rhs.span);
    let first = Arg { name: None, span: lhs.span, value: lhs };
    let (name, mut args) = match rhs.kind {
        ExprKind::Call(name, args) => (name, args),
        ExprKind::Var(name) => (name, vec![]),
        _ => {
            return Err(ParseError {
                message: "the right side of `|>` must be a function name or call".into(),
                span: rhs.span,
            })
        }
    };
    args.insert(0, first);
    Ok(Expr { kind: ExprKind::Call(name, args), span })
}

/// Parse the code inside `{...}` of an interpolated string.
fn interp_part(part: StrPart, depth: usize) -> PResult<InterpPart> {
    match part {
        StrPart::Lit(s) => Ok(InterpPart::Lit(s)),
        StrPart::Code(code, offset) => {
            let mut tokens = lex(&code).map_err(|mut e| {
                e.span = Span::new(e.span.start + offset, e.span.end + offset);
                ParseError::from(e)
            })?;
            for t in &mut tokens {
                t.span = Span::new(t.span.start + offset, t.span.end + offset);
            }
            let nl = vec![false; tokens.len()];
            let mut p = Parser {
                tokens,
                nl,
                pos: 0,
                no_struct: false,
                label: None,
                depth,
                errors: vec![],
                first_on_line: vec![],
                col0: vec![],
            };
            let e = p.expr()?;
            if !p.at(&T::Eof) {
                return p.error("`}` to end the interpolation");
            }
            Ok(InterpPart::Expr(e))
        }
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
        assert!(matches!(&f.body.stmts[0], Stmt::Let { pat, .. } if pat.kind == PatKind::Bind("i".into(), true)));
        assert!(matches!(f.body.stmts[3], Stmt::Return { .. }));
    }

    #[test]
    fn paren_on_a_new_line_starts_a_new_expression() {
        let src = "fn f(a: i64, b: i64) -> (i64, i64) {
            let w = a * b
            (w, b)
        }";
        let f = &parse(src).unwrap().functions[0];
        assert_eq!(f.body.stmts.len(), 1);
        assert!(matches!(&f.body.tail.as_ref().unwrap().kind, ExprKind::Tuple(_)));
        assert!(parse("fn main() { g(1,\n 2) }").is_ok());
    }

    #[test]
    fn else_if_chain() {
        let src = "fn f(x: i64) -> i64 { if x < 0 { 0 } else if x < 10 { 1 } else { 2 } }";
        assert!(parse(src).is_ok());
    }

    #[test]
    fn pipeline_desugars_to_calls() {
        let p = parse("fn main() { 5 |> double |> add(1) }").unwrap();
        let tail = p.functions[0].body.tail.as_ref().unwrap();
        let ExprKind::Call(name, args) = &tail.kind else { panic!() };
        assert_eq!(name, "add");
        assert_eq!(args.len(), 2);
        assert!(matches!(&args[0].value.kind, ExprKind::Call(n, a) if n == "double" && a.len() == 1));
        assert!(parse("fn main() { 1 |> 2 }").is_err());
    }

    #[test]
    fn named_and_default_arguments() {
        let p = parse("fn f(a: i64, b: i64 = 2) {} fn main() { f(1, b: 3) }").unwrap();
        assert!(p.functions[0].params[1].default.is_some());
        let ExprKind::Call(_, args) = &p.functions[1].body.tail.as_ref().unwrap().kind else { panic!() };
        assert_eq!(args[1].name.as_deref(), Some("b"));
    }

    #[test]
    fn interpolation_and_defer() {
        let p = parse(r#"fn main() { defer print("bye") print("x = {x + 1}") }"#).unwrap();
        let body = &p.functions[0].body;
        assert!(matches!(body.stmts[0], Stmt::Defer { .. }));
        let ExprKind::Call(_, args) = &body.tail.as_ref().unwrap().kind else { panic!() };
        let ExprKind::Interp(parts) = &args[0].value.kind else { panic!() };
        let InterpPart::Expr(e) = &parts[1] else { panic!() };
        assert_eq!(e.span.start, 43, "spans inside interpolation point at the original source");
    }

    #[test]
    fn structs_enums_impls() {
        let src = "
            struct Point { x: i64, y: i64 }
            enum Shape { Circle(f64), Rect(f64, f64), Empty }
            impl Point {
                fn new(x: i64, y: i64) -> Point { Point { x, y } }
                fn len2(&self) -> i64 { self.x * self.x + self.y * self.y }
                fn bump(&mut self, by: i64 = 1) { self.x += by }
            }
            const LIMIT: i64 = 10
            fn main() {
                let mut p = Point::new(1, 2)
                p.bump()
                p.x = p.len2()
                if p.x > LIMIT { print(1) }
            }";
        let p = parse(src).unwrap();
        assert_eq!(p.structs[0].fields.len(), 2);
        assert_eq!(p.enums[0].variants[1].fields.len(), 2);
        assert_eq!(p.impls[0].methods[2].params[0].mode, ParamMode::RefMut);
        assert_eq!(p.consts[0].name, "LIMIT");
        let body = &p.functions[0].body;
        assert!(matches!(&body.stmts[2], Stmt::Assign { target, .. } if matches!(target.kind, ExprKind::Field(..))));
        // `if p.x > LIMIT {` must not read `LIMIT {` as a struct literal.
        assert!(matches!(body.tail.as_ref().unwrap().kind, ExprKind::If(..)));
    }

    #[test]
    fn match_patterns() {
        let src = "fn f(s: Shape) -> i64 {
            match s {
                Shape::Circle(r) if r > 1.0 => 1,
                Shape::Rect(_, h) => { 2 }
                Shape::Empty | Shape::Circle(_) => 3,
            }
        }
        fn g(n: i64) -> i64 { match n { 0 => 0, 1..=9 => 1, -5 => 2, x => x } }";
        let p = parse(src).unwrap();
        let ExprKind::Match(_, arms) = &p.functions[0].body.tail.as_ref().unwrap().kind else { panic!() };
        assert_eq!(arms.len(), 3);
        assert!(arms[0].guard.is_some());
        assert!(matches!(arms[2].pat.kind, PatKind::Or(_)));
        let ExprKind::Match(_, arms) = &p.functions[1].body.tail.as_ref().unwrap().kind else { panic!() };
        assert!(matches!(arms[1].pat.kind, PatKind::Range(1, 9, true)));
        assert!(matches!(arms[2].pat.kind, PatKind::Int(-5)));
        assert!(matches!(arms[3].pat.kind, PatKind::Bind(_, false)));
    }

    #[test]
    fn loops_and_compound_assignment() {
        let src = "fn main() {
            let mut t = 0
            for i in 0..10 { if i == 3 { continue } t += i }
            let x = loop { break t * 2 }
            t -= x as i64
        }";
        let body = parse(src).unwrap().functions[0].body.clone();
        assert!(matches!(body.stmts[1], Stmt::For { inclusive: false, .. }));
        assert!(matches!(&body.stmts[2], Stmt::Let { value, .. } if matches!(value.kind, ExprKind::Loop(..))));
        assert!(matches!(body.stmts[3], Stmt::Assign { op: Some(BinOp::Sub), .. }));
    }

    #[test]
    fn generics_tuples_try_and_if_let() {
        let src = "
            struct Pair[A, B] { a: A, b: B }
            enum Opt[T] { Some(T), None }
            impl[T] Opt[T] { fn get(self, d: T) -> T { d } }
            fn first[T](p: (T, T)) -> Result[T, str] {
                let (x, mut y) = p
                let v = parse(x)?.0
                if let Opt::Some(z) = make() { y = z } else { return Err(\"no\") }
                while let Some(n) = next() { print(n) }
                Ok(())
            }";
        let p = parse(src).unwrap();
        assert_eq!(p.structs[0].generics, vec!["A", "B"]);
        assert_eq!(p.impls[0].generics, vec!["T"]);
        let f = &p.functions[0];
        assert_eq!(f.generics, vec!["T"]);
        assert_eq!(f.params[0].ty.as_ref().unwrap().name, TypeExpr::TUPLE);
        assert_eq!(f.ret.as_ref().unwrap().args.len(), 2);
        assert!(matches!(&f.body.stmts[0], Stmt::Let { pat, .. } if matches!(pat.kind, PatKind::Tuple(_))));
        assert!(matches!(&f.body.stmts[1], Stmt::Let { value, .. }
            if matches!(&value.kind, ExprKind::Field(inner, n) if n == "0" && matches!(inner.kind, ExprKind::Try(_)))));
        assert!(matches!(&f.body.stmts[2], Stmt::Expr(e) if matches!(e.kind, ExprKind::Match(..))));
        assert!(matches!(&f.body.stmts[3], Stmt::Expr(e) if matches!(e.kind, ExprKind::Loop(..))));
        assert!(
            matches!(&f.body.tail.as_ref().unwrap().kind, ExprKind::Call(n, a) if n == "Ok" && matches!(a[0].value.kind, ExprKind::Tuple(ref t) if t.is_empty()))
        );
    }

    #[test]
    fn reports_errors_with_location() {
        let err = parse("fn main( { }").unwrap_err();
        assert_eq!(err.message, "expected a name, found `{`");
        assert_eq!(err.span.start, 9);
    }
}

#[cfg(test)]
mod batch3_tests {
    use super::*;

    #[test]
    fn bit_operator_precedence_matches_rust() {
        let p = parse("fn f() -> i64 { 1 | 2 ^ 3 & 4 << 5 + 6 }").unwrap();
        // `1 | (2 ^ (3 & (4 << (5 + 6))))`
        let tail = p.functions[0].body.tail.as_ref().unwrap();
        let ExprKind::Binary(BinOp::BitOr, _, r) = &tail.kind else { panic!("{tail:?}") };
        let ExprKind::Binary(BinOp::BitXor, _, r) = &r.kind else { panic!() };
        let ExprKind::Binary(BinOp::BitAnd, _, r) = &r.kind else { panic!() };
        let ExprKind::Binary(BinOp::Shl, _, r) = &r.kind else { panic!() };
        assert!(matches!(r.kind, ExprKind::Binary(BinOp::Add, ..)));
    }

    #[test]
    fn labels_chars_and_suffixes() {
        let src = "fn main() { 'outer: for i in 0..3 { 'inner: while true { break 'outer } continue 'outer } let c = 'x' let n = 7u8 match c { 'a'..='z' => 1, _ => 0 } }";
        let p = parse(src).unwrap();
        let body = &p.functions[0].body;
        let Stmt::For { label, body: fb, .. } = &body.stmts[0] else { panic!() };
        assert_eq!(label.as_deref(), Some("outer"));
        let Stmt::While { label, body: wb, .. } = &fb.stmts[0] else { panic!() };
        assert_eq!(label.as_deref(), Some("inner"));
        assert!(matches!(&wb.stmts[0], Stmt::Break { label: Some(l), .. } if l == "outer"));
        assert!(matches!(&fb.stmts[1], Stmt::Continue { label: Some(l), .. } if l == "outer"));
        assert!(matches!(&body.stmts[1], Stmt::Let { value, .. } if value.kind == ExprKind::Char('x')));
        assert!(matches!(&body.stmts[2], Stmt::Let { value, .. } if matches!(value.kind, ExprKind::Cast(..))));
        assert!(parse("fn main() { 'a: let x = 1 }").is_err());
    }
}
