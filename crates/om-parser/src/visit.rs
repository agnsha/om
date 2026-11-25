//! Read-only traversal of the AST, for tools (lints, `om audit`, fix-its) that
//! need to see every expression without repeating the tree's shape.
//!
//! When you add an AST node, add it here too; the matches are exhaustive on
//! purpose so the compiler points at this file.

use crate::ast::*;

/// Callbacks for [`walk_program`]; every method defaults to doing nothing.
pub trait Visitor {
    /// Called for every function and method, before its body.
    fn function(&mut self, _f: &Function) {}
    /// Called for every expression, outermost first.
    fn expr(&mut self, _e: &Expr) {}
    /// Called for every statement, before its parts.
    fn stmt(&mut self, _s: &Stmt) {}
    /// Called for every pattern (in `let`, `match` and `for`), outermost first.
    fn pattern(&mut self, _p: &Pattern) {}
    /// Called after a function's body has been walked.
    fn function_end(&mut self, _f: &Function) {}
}

pub fn walk_program(v: &mut impl Visitor, p: &Program) {
    for f in &p.functions {
        walk_function(v, f);
    }
    for imp in &p.impls {
        for m in &imp.methods {
            walk_function(v, m);
        }
    }
    for t in &p.traits {
        for m in &t.methods {
            walk_function(v, m);
        }
    }
    for c in &p.consts {
        walk_expr(v, &c.value);
    }
}

pub fn walk_function(v: &mut impl Visitor, f: &Function) {
    v.function(f);
    for p in &f.params {
        if let Some(d) = &p.default {
            walk_expr(v, d);
        }
    }
    walk_block(v, &f.body);
    v.function_end(f);
}

pub fn walk_block(v: &mut impl Visitor, b: &Block) {
    for s in &b.stmts {
        walk_stmt(v, s);
    }
    if let Some(t) = &b.tail {
        walk_expr(v, t);
    }
}

pub fn walk_stmt(v: &mut impl Visitor, s: &Stmt) {
    v.stmt(s);
    match s {
        Stmt::Let { pat, value, .. } => {
            walk_pattern(v, pat);
            walk_expr(v, value);
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(v, target);
            walk_expr(v, value);
        }
        Stmt::While { cond, body, .. } => {
            walk_expr(v, cond);
            walk_block(v, body);
        }
        Stmt::For { start, end, body, .. } => {
            walk_expr(v, start);
            walk_expr(v, end);
            walk_block(v, body);
        }
        Stmt::ForEach { pat, iter, body, .. } => {
            walk_pattern(v, pat);
            walk_expr(v, iter);
            walk_block(v, body);
        }
        Stmt::Return { value, .. } | Stmt::Break { value, .. } => {
            if let Some(e) = value {
                walk_expr(v, e);
            }
        }
        Stmt::Continue { .. } => {}
        Stmt::Defer { body, .. } => walk_expr(v, body),
        Stmt::Expr(e) => walk_expr(v, e),
    }
}

pub fn walk_expr(v: &mut impl Visitor, e: &Expr) {
    v.expr(e);
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Char(_)
        | ExprKind::Str(_)
        | ExprKind::Var(_) => {}
        ExprKind::Unary(_, a)
        | ExprKind::Field(a, _)
        | ExprKind::Ref(_, a)
        | ExprKind::Try(a)
        | ExprKind::Cast(a, _) => walk_expr(v, a),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::ListRepeat(a, b) => {
            walk_expr(v, a);
            walk_expr(v, b);
        }
        ExprKind::Call(_, args) => {
            for a in args {
                walk_expr(v, &a.value);
            }
        }
        ExprKind::MethodCall(recv, _, args) => {
            walk_expr(v, recv);
            for a in args {
                walk_expr(v, &a.value);
            }
        }
        ExprKind::StructLit(_, fields) => {
            for (_, x) in fields {
                walk_expr(v, x);
            }
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            for x in items {
                walk_expr(v, x);
            }
        }
        ExprKind::Interp(parts) => {
            for p in parts {
                if let InterpPart::Expr(x) = p {
                    walk_expr(v, x);
                }
            }
        }
        ExprKind::If(c, t, els) => {
            walk_expr(v, c);
            walk_block(v, t);
            if let Some(b) = els {
                walk_block(v, b);
            }
        }
        ExprKind::Match(scrut, arms) => {
            walk_expr(v, scrut);
            for arm in arms {
                walk_pattern(v, &arm.pat);
                if let Some(g) = &arm.guard {
                    walk_expr(v, g);
                }
                walk_expr(v, &arm.body);
            }
        }
        ExprKind::Loop(b, _) | ExprKind::Block(b) => walk_block(v, b),
    }
}

pub fn walk_pattern(v: &mut impl Visitor, p: &Pattern) {
    v.pattern(p);
    match &p.kind {
        PatKind::Tuple(subs) | PatKind::Variant(_, subs) | PatKind::Or(subs) => {
            for s in subs {
                walk_pattern(v, s);
            }
        }
        PatKind::Wild
        | PatKind::Bind(..)
        | PatKind::Int(_)
        | PatKind::Bool(_)
        | PatKind::Char(_)
        | PatKind::Range(..)
        | PatKind::CharRange(..) => {}
    }
}
