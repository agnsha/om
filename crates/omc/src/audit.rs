//! Effects and `om audit`.
//!
//! Om tracks two effects: `io` (printing) and `panic` (`panic`, `assert`,
//! `assert_eq`, `unwrap`, `expect`). A function may declare the effects it is
//! allowed to have with a `uses` clause:
//!
//! ```text
//! fn log(n: i64) uses io { print(n) }
//! fn add(a: i64, b: i64) -> i64 uses { a + b }   // pure: no effects at all
//! ```
//!
//! Effects are inferred through the call graph, and a function that declares
//! `uses` gets an error (E0901) if it can do anything it did not declare.
//! Arithmetic overflow and division by zero also panic, but are not tracked,
//! as in Rust.

use crate::json;
use om_lexer::Span;
use om_parser::ast::*;
use om_parser::visit::{self, Visitor};
use std::collections::{BTreeMap, BTreeSet};

pub const EFFECTS: [&str; 2] = ["io", "panic"];

/// Why a function has an effect: something it does itself, or a call.
#[derive(Debug, Clone)]
enum Cause {
    Direct(&'static str, Span, &'static str),
    Call(String, Span),
}

/// One function's effects.
#[derive(Debug, Clone)]
pub struct FnEffects {
    pub key: String,
    pub span: Span,
    pub declared: Option<Vec<(String, Span)>>,
    pub effects: BTreeSet<&'static str>,
    causes: Vec<Cause>,
}

/// Effects of every function and method in `p`, by key (`f` or `Type::m`).
pub fn infer(p: &Program) -> BTreeMap<String, FnEffects> {
    let mut fns: BTreeMap<String, FnEffects> = BTreeMap::new();
    let mut by_method: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let all =
        p.functions.iter().map(|f| (f.name.clone(), f)).chain(
            p.impls.iter().flat_map(|i| i.methods.iter().map(move |m| (format!("{}::{}", i.ty.name, m.name), m))),
        );
    let mut bodies = Vec::new();
    for (key, f) in all {
        if f.params.first().is_some_and(|p| p.name == "self") {
            by_method.entry(f.name.clone()).or_default().push(key.clone());
        }
        bodies.push((key, f));
    }
    let user_fns: BTreeSet<String> = bodies.iter().map(|(k, _)| k.clone()).collect();
    for (key, f) in &bodies {
        let mut c = Collect { causes: vec![], user_fns: &user_fns, by_method: &by_method };
        visit::walk_block(&mut c, &f.body);
        fns.insert(
            key.clone(),
            FnEffects {
                key: key.clone(),
                span: f.span,
                declared: f.uses.clone(),
                effects: BTreeSet::new(),
                causes: c.causes,
            },
        );
    }
    // Fixpoint over the call graph.
    loop {
        let mut changed = false;
        let snapshot: BTreeMap<String, BTreeSet<&'static str>> =
            fns.iter().map(|(k, v)| (k.clone(), v.effects.clone())).collect();
        for f in fns.values_mut() {
            for cause in &f.causes {
                let add: Vec<&'static str> = match cause {
                    Cause::Direct(e, ..) => vec![e],
                    Cause::Call(callee, _) => {
                        snapshot.get(callee).map(|s| s.iter().copied().collect()).unwrap_or_default()
                    }
                };
                for e in add {
                    changed |= f.effects.insert(e);
                }
            }
        }
        if !changed {
            return fns;
        }
    }
}

struct Collect<'a> {
    causes: Vec<Cause>,
    user_fns: &'a BTreeSet<String>,
    by_method: &'a BTreeMap<String, Vec<String>>,
}

impl Visitor for Collect<'_> {
    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Call(name, _) if self.user_fns.contains(name) => {
                self.causes.push(Cause::Call(name.clone(), e.span))
            }
            ExprKind::Call(name, _) => match name.as_str() {
                "print" => self.causes.push(Cause::Direct("io", e.span, "prints")),
                "panic" | "assert" | "assert_eq" => self.causes.push(Cause::Direct("panic", e.span, "can panic")),
                _ => {}
            },
            ExprKind::MethodCall(_, m, _) => {
                if let Some(keys) = self.by_method.get(m) {
                    // Without types, a method call may reach any method of that name.
                    for k in keys {
                        self.causes.push(Cause::Call(k.clone(), e.span));
                    }
                } else if m == "unwrap" || m == "expect" {
                    self.causes.push(Cause::Direct("panic", e.span, "can panic"));
                }
            }
            _ => {}
        }
    }
}

/// E0901 and E0902 errors for functions whose `uses` clause is wrong.
pub fn check_declared(p: &Program) -> Vec<(String, Span)> {
    let fns = infer(p);
    let mut out = Vec::new();
    for f in fns.values() {
        let Some(declared) = &f.declared else { continue };
        for (e, span) in declared {
            if !EFFECTS.contains(&e.as_str()) {
                out.push((format!("unknown effect `{e}`; the effects are `io` and `panic`"), *span));
            }
        }
        let allowed: BTreeSet<&str> = declared.iter().map(|(e, _)| e.as_str()).collect();
        let list = if declared.is_empty() {
            String::new()
        } else {
            format!(" {}", join(declared.iter().map(|(e, _)| e.as_str())))
        };
        for e in f.effects.iter().filter(|e| !allowed.contains(*e)) {
            let witness = f.causes.iter().find_map(|c| match c {
                Cause::Direct(x, span, what) if x == e => Some((format!("{what} here"), *span)),
                Cause::Call(callee, span) if fns.get(callee).is_some_and(|g| g.effects.contains(e)) => {
                    Some((format!("calls `{callee}`, which uses {e}"), *span))
                }
                _ => None,
            });
            if let Some((why, span)) = witness {
                out.push((format!("`{}` declares `uses{list}` but {why}", f.key), span));
            }
        }
    }
    out.sort_by_key(|(_, s)| s.start);
    out
}

fn join<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

/// The `om audit` report for a program.
pub fn report(p: &Program, file: &str, as_json: bool) -> String {
    let fns = infer(p);
    let runtime = "C runtime: malloc/realloc/free, stdio output, exit, libm math";
    if as_json {
        let items = fns.values().map(|f| {
            let mut o = json::Obj::new().str("fn", &f.key).raw("effects", &json::strings(f.effects.iter().copied()));
            if let Some(d) = &f.declared {
                o = o.raw("declared", &json::strings(d.iter().map(|(e, _)| e.as_str())));
            }
            o.finish()
        });
        return json::Obj::new()
            .str("file", file)
            .raw("functions", &json::array(items))
            .num("unsafe_blocks", 0)
            .num("build_time_code", 0)
            .raw("dependencies", "[]")
            .str("foreign", runtime)
            .finish();
    }
    let width = fns.keys().map(|k| k.len()).max().unwrap_or(0).max(8);
    let mut out = format!("om audit: {file}\n");
    for f in fns.values() {
        let eff = if f.effects.is_empty() { "pure".to_string() } else { join(f.effects.iter().copied()) };
        let decl = match &f.declared {
            Some(d) if d.is_empty() => "  (declared: pure)".to_string(),
            Some(d) => format!("  (declared: {})", join(d.iter().map(|(e, _)| e.as_str()))),
            None => String::new(),
        };
        out.push_str(&format!("  {:width$}  {eff}{decl}\n", f.key));
    }
    out.push_str("unsafe code: none in this file (raw memory is used only inside the bundled Vec, String and Box)\n");
    out.push_str("code run at build time: none (no build scripts, macros or compile-time evaluation)\n");
    out.push_str("dependencies: none (one file; the prelude ships with the compiler)\n");
    out.push_str(&format!("foreign code: the {runtime}\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errors(src: &str) -> Vec<String> {
        check_declared(&om_parser::parse(src).unwrap()).into_iter().map(|(m, _)| m).collect()
    }

    #[test]
    fn infers_through_calls() {
        let p = om_parser::parse(
            "fn a() { print(1) } fn b() { a() } fn c(x: Option[i64]) -> i64 { x.unwrap() } fn d() -> i64 { 1 }",
        )
        .unwrap();
        let fx = infer(&p);
        assert_eq!(fx["b"].effects.iter().copied().collect::<Vec<_>>(), ["io"]);
        assert_eq!(fx["c"].effects.iter().copied().collect::<Vec<_>>(), ["panic"]);
        assert!(fx["d"].effects.is_empty());
    }

    #[test]
    fn checks_declared_effects() {
        assert!(errors("fn a() uses io { print(1) } fn main() { a() }").is_empty());
        assert_eq!(errors("fn a() uses { print(1) } fn main() {}"), ["`a` declares `uses` but prints here"]);
        assert_eq!(
            errors("fn a() { panic(\"x\") } fn b() uses io { a() } fn main() {}"),
            ["`b` declares `uses io` but calls `a`, which uses panic"]
        );
        assert_eq!(
            errors("fn a() uses net {} fn main() {}"),
            ["unknown effect `net`; the effects are `io` and `panic`"]
        );
    }

    #[test]
    fn audit_report_lists_functions() {
        let p = om_parser::parse("fn main() uses io { print(1) } fn f() -> i64 { 2 }").unwrap();
        let r = report(&p, "x.om", false);
        assert!(r.contains("main      io  (declared: io)"), "{r}");
        assert!(r.contains("f         pure"), "{r}");
        assert!(report(&p, "x.om", true).starts_with(r#"{"file":"x.om","functions":[{"fn":"f","effects":[]}"#));
    }
}
