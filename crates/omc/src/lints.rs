//! Checks that are errors in Om but are not about types: bare `unwrap()`
//! outside tests (E0900) and `uses` effect declarations (E0901, E0902).

use crate::Diagnostic;
use om_parser::ast::*;
use om_parser::visit::{self, Visitor};

/// True for test functions, which `om test` runs and which may `unwrap`.
pub fn is_test(f: &Function) -> bool {
    f.name.starts_with("test_")
}

pub fn check(p: &Program) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut v = Unwraps { in_test: false, found: vec![] };
    for f in &p.functions {
        v.in_test = is_test(f);
        visit::walk_function(&mut v, f);
    }
    v.in_test = false;
    for imp in &p.impls {
        for m in &imp.methods {
            visit::walk_function(&mut v, m);
        }
    }
    for span in v.found {
        out.push(Diagnostic::error(
            "bare `unwrap()` outside a test; use `?`, `unwrap_or`, `match`, or `expect(\"why it cannot fail\")`",
            Some(span),
        ));
    }
    for (msg, span) in crate::audit::check_declared(p) {
        out.push(Diagnostic::error(msg, Some(span)));
    }
    out
}

struct Unwraps {
    in_test: bool,
    found: Vec<om_lexer::Span>,
}

impl Visitor for Unwraps {
    fn expr(&mut self, e: &Expr) {
        if let ExprKind::MethodCall(_, m, args) = &e.kind {
            if m == "unwrap" && args.is_empty() && !self.in_test {
                self.found.push(e.span);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unwrap_is_an_error_outside_tests() {
        let src = "fn get() -> Option[i64] { Some(1) }\nfn main() { print(get().unwrap()) }\nfn test_get() { assert(get().unwrap() == 1) }\n";
        let d = crate::check(src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code, "E0900");
        assert_eq!(d[0].fixes[0].text, ".expect(\"TODO: why this cannot fail\")");
        assert!(crate::check("fn main() { print(Some(1).expect(\"literal\")) }").is_empty());
    }
}
