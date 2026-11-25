//! `om doc`: the real API, straight from the compiler, so agents look names up
//! instead of guessing them.
//!
//! * `om doc` lists everything in the prelude in a few lines.
//! * `om doc Option` shows a type with its variants, fields and methods.
//! * `om doc Option.unwrap` (or `Option::unwrap`) shows one signature and its docs.
//! * `om doc Ord` shows a trait and the types that implement it.
//! * `--file app.om` also searches your program.
//!
//! An unknown name prints the closest real ones and exits with status 1.

use crate::suggest::{prelude, similar, BUILTIN_TYPES};
use om_parser::ast::*;
use std::collections::BTreeSet;

/// `T`, `Option[T]`, `(A, B)`, `()`.
pub fn type_str(t: &TypeExpr) -> String {
    let args: Vec<String> = t.args.iter().map(type_str).collect();
    if t.name == TypeExpr::TUPLE {
        return match args.len() {
            1 => format!("({},)", args[0]),
            _ => format!("({})", args.join(", ")),
        };
    }
    if args.is_empty() {
        t.name.clone()
    } else {
        format!("{}[{}]", t.name, args.join(", "))
    }
}

fn literal(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float(x) => format!("{x:?}"),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Char(c) => format!("{c:?}"),
        ExprKind::Str(s) => format!("{s:?}"),
        ExprKind::Unary(UnOp::Neg, x) => format!("-{}", literal(x)),
        ExprKind::Var(v) => v.clone(),
        _ => "...".into(),
    }
}

/// One-line signature: `fn name[T: Eq](&self, x: &mut i64, n: i64 = 1) -> T uses io`.
pub fn signature(f: &Function) -> String {
    let generics = if f.generics.is_empty() {
        String::new()
    } else {
        let gs: Vec<String> = f
            .generics
            .iter()
            .map(|g| {
                let bs: Vec<&str> = f.bounds.iter().filter(|b| &b.param == g).map(|b| b.trait_name.as_str()).collect();
                if bs.is_empty() {
                    g.clone()
                } else {
                    format!("{g}: {}", bs.join(" + "))
                }
            })
            .collect();
        format!("[{}]", gs.join(", "))
    };
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let m = if p.mutable { "mut " } else { "" };
            let Some(ty) = &p.ty else {
                return match p.mode {
                    ParamMode::Value => format!("{m}self"),
                    ParamMode::Ref => "&self".into(),
                    ParamMode::RefMut => "&mut self".into(),
                };
            };
            let r = match p.mode {
                ParamMode::Value => "",
                ParamMode::Ref => "&",
                ParamMode::RefMut => "&mut ",
            };
            let d = p.default.as_ref().map(|d| format!(" = {}", literal(d))).unwrap_or_default();
            format!("{m}{}: {r}{}{d}", p.name, type_str(ty))
        })
        .collect();
    let ret = f.ret.as_ref().map(|t| format!(" -> {}", type_str(t))).unwrap_or_default();
    let uses = match &f.uses {
        Some(u) => format!(" uses {}", u.iter().map(|(e, _)| e.as_str()).collect::<Vec<_>>().join(", ")),
        None => String::new(),
    };
    format!("fn {}{generics}({}){ret}{uses}", f.name, params.join(", "))
}

/// `///` comment lines directly above byte offset `pos`.
fn doc_comment(src: &str, pos: usize) -> String {
    let before = &src[..pos.min(src.len())];
    let mut lines: Vec<&str> = Vec::new();
    for line in before.lines().rev().skip(usize::from(!before.ends_with('\n'))) {
        match line.trim().strip_prefix("///") {
            Some(text) => lines.push(text.trim()),
            None => break,
        }
    }
    lines.reverse();
    lines.join("\n")
}

const BUILTINS: [(&str, &str, &str); 3] = [
    ("print", "fn print(value)", "Print any value or interpolated string (`\"x = {x}\"`) and a newline."),
    ("panic", "fn panic(message = \"explicit panic\")", "Print `panic: message` to stderr and exit with status 101."),
    ("assert", "fn assert(cond: bool, message = \"assertion failed\")", "Panic with `message` if `cond` is false."),
];

/// Every program the lookup searches: the prelude, then the user's file.
struct Sources<'a> {
    items: Vec<(&'a Program, &'a str)>,
}

/// Look `query` up. `Err` holds the "not found" text with suggestions.
pub fn lookup(query: &str, user: Option<(&Program, &str)>) -> Result<String, String> {
    let psrc = om_codegen::prelude_source();
    let mut s = Sources { items: vec![(prelude(), psrc.as_str())] };
    if let Some(u) = user {
        s.items.push(u);
    }
    let q = query.replace("::", ".");
    if let Some((ty, m)) = q.split_once('.') {
        return method(&s, ty, m).ok_or_else(|| not_found(&s, query));
    }
    if let Some((_, sig, doc)) = BUILTINS.iter().find(|(n, ..)| *n == q) {
        return Ok(format!("{sig}\n  built-in. {doc}\n"));
    }
    if let Some(out) = type_doc(&s, &q) {
        return Ok(out);
    }
    if let Some(out) = trait_doc(&s, &q) {
        return Ok(out);
    }
    for (p, src) in &s.items {
        if let Some(f) = p.functions.iter().find(|f| f.name == q) {
            return Ok(with_doc(signature(f), doc_comment(src, f.span.start)));
        }
    }
    Err(not_found(&s, query))
}

fn with_doc(sig: String, doc: String) -> String {
    if doc.is_empty() {
        format!("{sig}\n")
    } else {
        format!("{sig}\n  {}\n", doc.replace('\n', "\n  "))
    }
}

fn methods_of<'a>(s: &'a Sources, ty: &str) -> Vec<(&'a Function, &'a str, Option<&'a str>)> {
    let mut out = Vec::new();
    for (p, src) in &s.items {
        for imp in p.impls.iter().filter(|i| i.ty.name == ty) {
            let tn = imp.trait_name.as_ref().map(|(n, _)| n.as_str());
            for m in &imp.methods {
                out.push((m, *src, tn));
            }
            // Provided trait methods the impl did not override.
            if let Some(tn) = tn {
                for (tp, tsrc) in &s.items {
                    for t in tp.traits.iter().filter(|t| t.name == tn) {
                        for m in
                            t.methods.iter().filter(|m| m.has_body && !imp.methods.iter().any(|x| x.name == m.name))
                        {
                            out.push((m, *tsrc, Some(tn)));
                        }
                    }
                }
            }
        }
    }
    out
}

fn method(s: &Sources, ty: &str, m: &str) -> Option<String> {
    if let Some((f, src, tn)) = methods_of(s, ty).into_iter().find(|(f, ..)| f.name == m) {
        let from = tn.map(|t| format!("  (from trait {t})")).unwrap_or_default();
        return Some(with_doc(format!("fn {ty}::{}{from}", &signature(f)[3..]), doc_comment(src, f.span.start)));
    }
    for (p, src) in &s.items {
        if let Some(t) = p.traits.iter().find(|t| t.name == ty) {
            if let Some(f) = t.methods.iter().find(|f| f.name == m) {
                return Some(with_doc(format!("fn {ty}::{}", &signature(f)[3..]), doc_comment(src, f.span.start)));
            }
        }
    }
    None
}

fn type_doc(s: &Sources, name: &str) -> Option<String> {
    let mut out = String::new();
    for (p, src) in &s.items {
        if let Some(st) = p.structs.iter().find(|x| x.name == name) {
            let g = if st.generics.is_empty() { String::new() } else { format!("[{}]", st.generics.join(", ")) };
            let fields: Vec<String> = st.fields.iter().map(|(f, t)| format!("{f}: {}", type_str(t))).collect();
            out = with_doc(format!("struct {name}{g} {{ {} }}", fields.join(", ")), doc_comment(src, st.span.start));
        }
        if let Some(en) = p.enums.iter().find(|x| x.name == name) {
            let g = if en.generics.is_empty() { String::new() } else { format!("[{}]", en.generics.join(", ")) };
            let vs: Vec<String> = en
                .variants
                .iter()
                .map(|v| {
                    if v.fields.is_empty() {
                        v.name.clone()
                    } else {
                        format!("{}({})", v.name, v.fields.iter().map(type_str).collect::<Vec<_>>().join(", "))
                    }
                })
                .collect();
            out = with_doc(format!("enum {name}{g} {{ {} }}", vs.join(", ")), doc_comment(src, en.span.start));
        }
    }
    if out.is_empty() && !BUILTIN_TYPES.contains(&name) {
        return None;
    }
    if out.is_empty() {
        out = format!("{name}  (built-in type)\n");
    }
    let methods = methods_of(s, name);
    let traits: BTreeSet<&str> = methods.iter().filter_map(|(_, _, t)| *t).collect();
    if !traits.is_empty() {
        out.push_str(&format!("  implements: {}\n", traits.into_iter().collect::<Vec<_>>().join(", ")));
    }
    let mut seen = BTreeSet::new();
    for (f, _, _) in methods {
        if seen.insert(&f.name) {
            out.push_str(&format!("  {}\n", signature(f)));
        }
    }
    Some(out)
}

fn trait_doc(s: &Sources, name: &str) -> Option<String> {
    let (t, src) = s.items.iter().find_map(|(p, src)| p.traits.iter().find(|t| t.name == name).map(|t| (t, *src)))?;
    let mut out = with_doc(format!("trait {name}"), doc_comment(src, t.span.start));
    for m in &t.methods {
        let req = if m.has_body { "" } else { "  (required)" };
        out.push_str(&format!("  {}{req}\n", signature(m)));
    }
    let mut impls: Vec<String> = s
        .items
        .iter()
        .flat_map(|(p, _)| p.impls.iter())
        .filter(|i| i.trait_name.as_ref().is_some_and(|(n, _)| n == name))
        .map(|i| type_str(&i.ty))
        .collect();
    impls.sort();
    impls.dedup();
    if !impls.is_empty() {
        out.push_str(&format!("  implemented by: {}\n", impls.join(", ")));
    }
    Some(out)
}

fn all_names(s: &Sources) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BUILTINS.iter().map(|b| b.0.to_string()).collect();
    names.extend(BUILTIN_TYPES.iter().map(|t| t.to_string()));
    for (p, _) in &s.items {
        names.extend(p.functions.iter().map(|f| f.name.clone()));
        names.extend(p.structs.iter().map(|x| x.name.clone()));
        names.extend(p.enums.iter().map(|x| x.name.clone()));
        names.extend(p.traits.iter().map(|x| x.name.clone()));
        for imp in &p.impls {
            for m in &imp.methods {
                names.insert(format!("{}.{}", imp.ty.name, m.name));
            }
        }
    }
    names
}

fn not_found(s: &Sources, query: &str) -> String {
    let q = query.replace("::", ".");
    let names = all_names(s);
    // For `Type.method`, compare methods of that type first.
    let mut close = match q.split_once('.') {
        Some((ty, m)) => {
            let ms: Vec<String> =
                names.iter().filter_map(|n| n.strip_prefix(&format!("{ty}.")).map(str::to_string)).collect();
            similar(m, &ms).into_iter().map(|m| format!("{ty}.{m}")).collect()
        }
        None => vec![],
    };
    if close.is_empty() {
        close = similar(&q, &names);
    }
    if close.is_empty() {
        format!("no item named `{query}`; run `om doc` for the list")
    } else {
        format!(
            "no item named `{query}`; did you mean {}?",
            close.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ")
        )
    }
}

/// `om doc` with no name: everything in the prelude, compactly.
pub fn overview(user: Option<(&Program, &str)>) -> String {
    let psrc = om_codegen::prelude_source();
    let mut s = Sources { items: vec![(prelude(), psrc.as_str())] };
    if let Some(u) = user {
        s.items.push(u);
    }
    let mut out = String::from("built-in functions: print(value), panic(message), assert(cond, message)\n");
    out.push_str(&format!("built-in types: {}\n", BUILTIN_TYPES.join(" ")));
    let mut types = BTreeSet::new();
    let mut traits = BTreeSet::new();
    let mut fns = BTreeSet::new();
    for (p, _) in &s.items {
        types.extend(p.structs.iter().map(|x| x.name.clone()).chain(p.enums.iter().map(|x| x.name.clone())));
        traits.extend(p.traits.iter().map(|x| x.name.clone()));
        fns.extend(p.functions.iter().map(|f| f.name.clone()));
    }
    out.push_str(&format!("types: {}\n", types.into_iter().collect::<Vec<_>>().join(" ")));
    out.push_str(&format!("traits: {}\n", traits.into_iter().collect::<Vec<_>>().join(" ")));
    if !fns.is_empty() {
        out.push_str(&format!("functions: {}\n", fns.into_iter().collect::<Vec<_>>().join(" ")));
    }
    out.push_str("details: om doc NAME, e.g. `om doc Option`, `om doc i64.pow`, `om doc Ord`\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_methods_types_and_traits() {
        let unwrap = lookup("Option.unwrap", None).unwrap();
        assert!(unwrap.starts_with("fn Option::unwrap(self) -> T"), "{unwrap}");
        assert_eq!(lookup("Option::unwrap", None).unwrap(), unwrap);
        let opt = lookup("Option", None).unwrap();
        assert!(opt.starts_with("enum Option[T] { Some(T), None }"), "{opt}");
        assert!(opt.contains("fn unwrap_or(self, default: T) -> T"), "{opt}");
        let ord = lookup("Ord", None).unwrap();
        assert!(ord.contains("fn cmp(&self, other: &Self) -> Ordering  (required)"), "{ord}");
        assert!(ord.contains("implemented by:") && ord.contains("i64"), "{ord}");
        assert!(lookup("i64", None).unwrap().contains("fn pow(self, exp: u32) -> i64"));
        assert!(lookup("print", None).unwrap().contains("built-in"));
    }

    #[test]
    fn unknown_names_get_suggestions() {
        assert_eq!(
            lookup("Option.unwarp", None).unwrap_err(),
            "no item named `Option.unwarp`; did you mean `Option.unwrap`?"
        );
        assert!(lookup("Optoin", None).unwrap_err().contains("`Option`"));
    }

    #[test]
    fn searches_the_user_program() {
        let src = "/// A point.\nstruct P { x: i64 }\nimpl P {\n    /// Twice x.\n    fn double(&self) -> i64 { self.x * 2 }\n}\nfn main() {}\n";
        let p = om_parser::parse(src).unwrap();
        assert_eq!(lookup("P.double", Some((&p, src))).unwrap(), "fn P::double(&self) -> i64\n  Twice x.\n");
        assert!(lookup("P", Some((&p, src))).unwrap().starts_with("struct P { x: i64 }\n  A point.\n"));
    }

    #[test]
    fn signatures_round_trip_the_details() {
        let p =
            om_parser::parse("fn f[T: Eq + Ord](mut a: T, b: &mut i64, c: str = \"x\") -> (T, i64) uses io { (a, b) }")
                .unwrap();
        assert_eq!(
            signature(&p.functions[0]),
            "fn f[T: Eq + Ord](mut a: T, b: &mut i64, c: str = \"x\") -> (T, i64) uses io"
        );
    }
}
