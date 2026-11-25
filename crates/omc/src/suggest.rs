//! Turn compiler messages into fixes: did-you-mean for unknown names, exact
//! edits for common mistakes, and rewrites of Rust habits that agents bring to
//! Om (`println!`, `pub`, `String`, `Vec<T>`, `.clone()`, `#[derive]`).
//!
//! Fixes are computed from the message, its span, the source and the parsed
//! program, so the type checker stays free of presentation code.

use crate::codes::classify;
use crate::{Applicability, Diagnostic, Fix};
use om_lexer::Span;
use om_parser::ast::*;
use om_parser::visit::{self, Visitor};
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

/// The prelude, parsed once per process.
pub fn prelude() -> &'static Program {
    static P: OnceLock<Program> = OnceLock::new();
    P.get_or_init(|| om_parser::parse(&om_codegen::prelude_source()).unwrap_or_default())
}

pub const BUILTIN_FNS: [&str; 3] = ["print", "panic", "assert"];
pub const BUILTIN_TYPES: [&str; 15] =
    ["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64", "bool", "char", "str"];

/// Names known in a program, for suggestions.
pub struct Context<'a> {
    pub src: &'a str,
    pub program: &'a Program,
    pub functions: BTreeSet<String>,
    pub types: BTreeSet<String>,
    pub traits: BTreeSet<String>,
    pub fields: HashMap<String, Vec<String>>,
    pub variants: HashMap<String, Vec<String>>,
    /// Methods by type name (impl methods plus methods of implemented traits).
    pub methods: HashMap<String, BTreeSet<String>>,
    pub consts: BTreeSet<String>,
}

impl<'a> Context<'a> {
    pub fn new(src: &'a str, program: &'a Program) -> Context<'a> {
        let mut c = Context {
            src,
            program,
            functions: BUILTIN_FNS.iter().map(|s| s.to_string()).collect(),
            types: BUILTIN_TYPES.iter().map(|s| s.to_string()).collect(),
            traits: BTreeSet::new(),
            fields: HashMap::new(),
            variants: HashMap::new(),
            methods: HashMap::new(),
            consts: BTreeSet::new(),
        };
        for p in [prelude(), program] {
            for f in &p.functions {
                c.functions.insert(f.name.clone());
            }
            for s in &p.structs {
                c.types.insert(s.name.clone());
                c.fields.insert(s.name.clone(), s.fields.iter().map(|(f, _)| f.clone()).collect());
            }
            for e in &p.enums {
                c.types.insert(e.name.clone());
                c.variants.insert(e.name.clone(), e.variants.iter().map(|v| v.name.clone()).collect());
            }
            for t in &p.traits {
                c.traits.insert(t.name.clone());
            }
            for k in &p.consts {
                c.consts.insert(k.name.clone());
            }
        }
        let traits: Vec<&TraitDef> = prelude().traits.iter().chain(&program.traits).collect();
        for p in [prelude(), program] {
            for imp in &p.impls {
                let set = c.methods.entry(imp.ty.name.clone()).or_default();
                for m in &imp.methods {
                    set.insert(m.name.clone());
                    if imp.trait_name.is_none() && m.params.first().is_none_or(|p| p.name != "self") {
                        c.functions.insert(format!("{}::{}", imp.ty.name, m.name));
                    }
                }
                if let Some((tn, _)) = &imp.trait_name {
                    if let Some(t) = traits.iter().find(|t| &t.name == tn) {
                        set.extend(t.methods.iter().map(|m| m.name.clone()));
                    }
                }
            }
        }
        c
    }

    fn text(&self, span: Span) -> &'a str {
        let s = clamp(self.src, span.start);
        let e = clamp(self.src, span.end).max(s);
        &self.src[s..e]
    }
}

fn clamp(src: &str, pos: usize) -> usize {
    let mut p = pos.min(src.len());
    while !src.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Edit distance between two names, ignoring case and `_` when that makes them equal.
pub fn distance(a: &str, b: &str) -> usize {
    let norm = |s: &str| s.chars().filter(|&c| c != '_').flat_map(char::to_lowercase).collect::<String>();
    if norm(a) == norm(b) {
        return 0;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(!a[i - 1].eq_ignore_ascii_case(&b[j - 1]));
            cur[j] = sub.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Up to three names close to `name`, closest first.
pub fn similar<'b>(name: &str, candidates: impl IntoIterator<Item = &'b String>) -> Vec<String> {
    let limit = (name.chars().count() / 3).max(1);
    let prefix = |c: &str| name.chars().zip(c.chars()).take_while(|(a, b)| a == b).count();
    // Closest first; among equally close names, the longest shared prefix.
    let mut scored: Vec<(usize, std::cmp::Reverse<usize>, &String)> = candidates
        .into_iter()
        .filter(|c| c.as_str() != name && !c.starts_with('$'))
        .map(|c| (distance(name, c), std::cmp::Reverse(prefix(c)), c))
        .filter(|(d, _, c)| *d <= limit || (c.len() > 3 && (c.starts_with(name) || name.starts_with(c.as_str()))))
        .collect();
    scored.sort();
    scored.into_iter().take(3).map(|(_, _, c)| c.clone()).collect()
}

/// Rust names with a direct Om equivalent.
fn rust_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "String" => "str",
        "i128" => "i64",
        "u128" => "u64",
        "PartialEq" => "Eq",
        "PartialOrd" => "Ord",
        _ => return None,
    })
}

/// Methods Rust needs to turn borrowed or owned data into values. Om values are
/// already plain values, so the call can simply go.
const NOOP_METHODS: [&str; 7] = ["clone", "to_string", "to_owned", "as_str", "as_ref", "borrow", "into"];

/// Add fixes to `d` (and, for Rust habits, a clearer message).
pub fn enrich(d: &mut Diagnostic, cx: &Context) {
    let Some(span) = d.span else { return };
    let (_, caps) = classify(&d.message);
    let cap = |i: usize| caps.get(i).map(String::as_str).unwrap_or("");
    let mut fixes = Vec::new();
    let mut add = |app: Applicability, span: Span, text: &str, label: String| {
        fixes.push(Fix { applicability: app, span, text: text.to_string(), label });
    };
    use Applicability::*;
    'arms: {
        match d.code {
            "E0100" => syntax(d, cx, span, cap(0), cap(1), &mut add),
            "E0105" if d.message.starts_with("unmatched") => {
                add(Auto, span, "}}", "write `}}` for a literal `}`".into())
            }
            "E0105" => add(Maybe, span, "{{}}", "write `{{}}` for literal braces, or put an expression inside".into()),
            "E0106" => macro_call(cx, span, &mut add),
            "E0107" => {
                let del = whole_line(cx.src, span);
                add(Auto, del, "", "delete the attribute".into());
            }
            "E0200" => {
                let name = cap(0);
                let mut cands: BTreeSet<String> = locals_before(cx.program, span.start);
                cands.extend(cx.consts.iter().cloned());
                let alt = match name {
                    "null" | "nil" | "NULL" => Some("None".to_string()),
                    _ => similar(name, &cands).into_iter().next(),
                };
                if let Some(alt) = alt {
                    if cx.text(span) == name {
                        add(Maybe, span, &alt, format!("did you mean `{alt}`?"));
                    }
                }
            }
            "E0201" => {
                let name = cap(0);
                let at = Span::new(span.start, span.start + name.len());
                if cx.text(at) != name {
                    break 'arms;
                }
                if name == "String::from" || name == "String::new" {
                    // `String::from("x")` is just `"x"` in Om.
                    let inner = cx.text(span).strip_prefix(name).unwrap_or("");
                    let lit = inner.trim().strip_prefix('(').and_then(|s| s.strip_suffix(')')).unwrap_or("").trim();
                    let lit = if lit.is_empty() { "\"\"" } else { lit };
                    add(Auto, span, lit, "Om strings need no `String::from`".into());
                } else if name == "println" || name == "print_line" || name == "puts" || name == "echo" {
                    add(Auto, at, "print", "Om's `print` adds the newline".into());
                } else if let Some(alt) = similar(name, &cx.functions).into_iter().next() {
                    add(Maybe, at, &alt, format!("did you mean `{alt}`?"));
                }
            }
            "E0202" | "E0203" | "E0204" => {
                let name = cap(0);
                let at = Span::new(span.start, span.start + name.len());
                if cx.text(at) != name {
                    break 'arms;
                }
                if let Some(alt) = rust_type(name) {
                    add(Auto, at, alt, format!("Om calls this `{alt}`"));
                    break 'arms;
                }
                let pool = if d.code == "E0204" { &cx.traits } else { &cx.types };
                if let Some(alt) = similar(name, pool).into_iter().next() {
                    add(Maybe, at, &alt, format!("did you mean `{alt}`?"));
                }
            }
            "E0205" => {
                let (ty, m) = (cap(0).trim_matches('`'), cap(1));
                let base = ty.split(['<', '[']).next().unwrap_or(ty);
                let text = cx.text(span);
                if NOOP_METHODS.contains(&m) {
                    let call = format!(".{m}()");
                    if text.ends_with(&call) {
                        let at = Span::new(span.end - call.len(), span.end);
                        add(Auto, at, "", format!("Om values are plain values; `.{m}()` is not needed"));
                        break 'arms;
                    }
                }
                let Some(pool) = cx.methods.get(base) else { break 'arms };
                if let Some(alt) = similar(m, pool).into_iter().next() {
                    let needle = format!(".{m}");
                    if let Some(i) = text.rfind(&needle) {
                        let at = Span::new(span.start + i + 1, span.start + i + needle.len());
                        add(Maybe, at, &alt, format!("did you mean `{alt}`?"));
                    }
                }
            }
            "E0206" => {
                let (ty, f) = (cap(0).trim_matches('`'), cap(1));
                let base = ty.split(['<', '[']).next().unwrap_or(ty);
                let Some(pool) = cx.fields.get(base) else { break 'arms };
                let pool: Vec<String> = pool.clone();
                if let Some(alt) = similar(f, &pool).into_iter().next() {
                    if cx.text(span).ends_with(f) {
                        add(Maybe, Span::new(span.end - f.len(), span.end), &alt, format!("did you mean `{alt}`?"));
                    }
                }
            }
            "E0207" => {
                let (ty, v) = (cap(0), cap(1));
                let v = v.rsplit("::").next().unwrap_or(v);
                let base = ty.split(['<', '[']).next().unwrap_or(ty);
                let Some(pool) = cx.variants.get(base) else { break 'arms };
                let pool: Vec<String> = pool.clone();
                if let Some(alt) = similar(v, &pool).into_iter().next() {
                    if let Some(i) = cx.text(span).find(v) {
                        add(
                            Maybe,
                            Span::new(span.start + i, span.start + i + v.len()),
                            &alt,
                            format!("did you mean `{alt}`?"),
                        );
                    }
                }
            }
            "E0300" => mismatched_operands(cx, span, cap(0), &mut add),
            "E0301" => {
                // `let x: f64 = 1` -> `1.0`
                let t = cx.text(span);
                if (cap(0) == "f64" || cap(0) == "f32")
                    && !t.is_empty()
                    && t.chars().all(|c| c.is_ascii_digit() || c == '_')
                {
                    add(Auto, Span::new(span.end, span.end), ".0", "write a float literal".into());
                }
            }
            "E0302" if d.message.contains("returns nothing, but its body ends with a ") => {
                let name = cap(0);
                let ty = cap(1);
                if let Some(f) = find_fn(cx.program, name, span.start) {
                    if let Some(close) = cx.src[..clamp(cx.src, f.body.span.start)].rfind(')') {
                        if f.ret.is_none() && f.span.start < close {
                            add(
                                Maybe,
                                Span::new(close + 1, close + 1),
                                &format!(" -> {ty}"),
                                format!("declare that `{name}` returns {ty}"),
                            );
                        }
                    }
                }
            }
            "E0500" => {
                let missing = cap(0).split(" (add an arm").next().unwrap_or("").trim();
                let text = cx.text(span);
                if !missing.is_empty() && text.ends_with('}') {
                    let at = Span::new(span.end - 1, span.end - 1);
                    let sep = if text[..text.len() - 1].trim_end().ends_with([',', '{', '}']) { "" } else { "," };
                    add(
                        Placeholder,
                        at,
                        &format!("{sep} {missing} => panic(\"TODO\") "),
                        format!("add an arm for `{missing}`"),
                    );
                }
            }
            "E0600" => {
                let name = cap(0);
                if let Some(pat) = binding_site(cx.program, name, span.start) {
                    add(Auto, Span::new(pat.start, pat.start), "mut ", format!("declare `{name}` with `let mut`"));
                }
            }
            "E0601" => {
                if let Some(f) = enclosing_fn(cx.program, span.start) {
                    if let Some(p) = f.params.first().filter(|p| p.name == "self" && p.mode == ParamMode::Ref) {
                        add(Maybe, p.span, "&mut self", "take `&mut self`".into());
                    }
                }
            }
            "E0603" => add(Auto, Span::new(span.start, span.start), "&mut ", "pass it as `&mut`".into()),
            "E0604" => add(Auto, Span::new(span.start, span.start), "&", "pass it as `&`".into()),
            "E0605" => {
                let t = cx.text(span);
                let n = if t.starts_with("&mut ") {
                    5
                } else if t.starts_with('&') {
                    1
                } else {
                    0
                };
                if n > 0 {
                    add(Auto, Span::new(span.start, span.start + n), "", "remove the `&`".into());
                }
            }
            "E0401" => {
                let text = cx.text(span);
                if let Some(head) = text.strip_suffix(')') {
                    let sep = if head.trim_end().ends_with('(') { "" } else { ", " };
                    let at = Span::new(span.end - 1, span.end - 1);
                    add(Placeholder, at, &format!("{sep}{}: TODO", cap(0)), format!("pass `{}`", cap(0)));
                }
            }
            "E0405" if d.message.starts_with("missing field") => {
                let text = cx.text(span);
                if let Some(inner) = text.strip_suffix('}') {
                    let names: Vec<String> = cap(1).split(", ").map(|f| format!("{f}: TODO")).collect();
                    let inner = inner.trim_end();
                    let sep = if inner.ends_with('{') || inner.ends_with(',') { " " } else { ", " };
                    let at = Span::new(span.end - 1, span.end - 1);
                    add(Placeholder, at, &format!("{sep}{} ", names.join(", ")), "add the missing fields".into());
                }
            }
            "E0702" => missing_trait_method(cx, span, cap(0), cap(1), &mut add),
            "E0900" => {
                let call = ".unwrap()";
                if cx.text(span).ends_with(call) {
                    let at = Span::new(span.end - call.len(), span.end);
                    add(
                        Placeholder,
                        at,
                        ".expect(\"TODO: why this cannot fail\")",
                        "say why it cannot fail, or use `?`".into(),
                    );
                }
            }
            _ => {}
        }
    }
    // Rust keywords get their own code and message.
    if d.code == "E0100" {
        if let Some(kw) = ["pub", "use", "mod", "unsafe", "extern", "crate", "dyn", "mut"]
            .into_iter()
            .find(|k| cap(1) == format!("identifier `{k}`") || cap(1) == format!("`{k}`"))
        {
            let why = match kw {
                "pub" => "everything in a file is visible",
                "use" | "mod" | "extern" | "crate" => "a program is one file and the prelude is always available",
                "unsafe" => "Om has no unsafe code",
                "dyn" => "use a generic `[T: Trait]` instead",
                _ => "only `let mut` and `&mut` use it",
            };
            if kw != "mut" {
                d.code = "E0109";
                d.message = format!("Om has no `{kw}`: {why}");
            }
        }
    }
    d.fixes.extend(fixes);
}

/// Fixes for generic syntax errors: Rust keywords, `<T>` generics, `&str` types,
/// `->` in match arms and missing commas.
fn syntax(
    d: &Diagnostic,
    cx: &Context,
    span: Span,
    expected: &str,
    found: &str,
    add: &mut impl FnMut(Applicability, Span, &str, String),
) {
    use Applicability::*;
    let src = cx.src;
    let _ = d;
    let line_end = src[clamp(src, span.start)..].find('\n').map_or(src.len(), |i| span.start + i);
    match found {
        "identifier `pub`" | "identifier `unsafe`" => {
            let kw_end = span.start + if found.contains("pub") { 3 } else { 6 };
            let mut end = kw_end;
            if src[end..].starts_with("(crate)") {
                end += 7;
            }
            while src[end..].starts_with(' ') {
                end += 1;
            }
            add(Auto, Span::new(span.start, end), "", "delete it".into());
        }
        "identifier `use`" | "identifier `mod`" | "identifier `extern`" => {
            let stmt_end = src[span.start..line_end].find(';').map_or(line_end, |i| span.start + i + 1);
            let del = whole_line(src, Span::new(span.start, stmt_end));
            add(Auto, del, "", "delete it".into());
        }
        "`<`" => {
            // `Vec<T>` / `fn f<T>` -> `[T]`, when the brackets close on this line.
            let before = src[..span.start].trim_end();
            if !before.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                return;
            }
            let mut depth = 0;
            let bytes = src.as_bytes();
            let mut i = span.start;
            while i < line_end {
                match bytes[i] {
                    b'<' => depth += 1,
                    b'>' if i > 0 && (bytes[i - 1] == b'-' || bytes[i - 1] == b'=') => {}
                    b'>' => {
                        depth -= 1;
                        if depth == 0 {
                            add(
                                Auto,
                                Span::new(span.start, span.start + 1),
                                "[",
                                "Om writes type arguments in `[...]`".into(),
                            );
                            add(Auto, Span::new(i, i + 1), "]", "Om writes type arguments in `[...]`".into());
                            return;
                        }
                    }
                    b'{' | b';' | b'(' if depth == 0 => return,
                    _ => {}
                }
                i += 1;
            }
        }
        "`&`" if expected == "a name" => {
            let mut end = span.start + 1;
            for word in ["'static ", "'a ", "mut "] {
                if src[end..].starts_with(word) {
                    end += word.len();
                }
            }
            add(Auto, Span::new(span.start, end), "", "Om types have no `&`; values are passed by value".into());
        }
        "`->`" if expected == "`=>`" => add(Auto, span, "=>", "match arms use `=>`".into()),
        _ if expected == "`,` between match arms" => {
            let end = src[..span.start].trim_end().len();
            add(Auto, Span::new(end, end), ",", "add a comma after the arm".into());
        }
        _ => {}
    }
}

/// `println!("x = {}", x)` -> `print("x = {x}")`, and friends.
fn macro_call(cx: &Context, bang: Span, add: &mut impl FnMut(Applicability, Span, &str, String)) {
    use Applicability::*;
    let src = cx.src;
    if bang.start > src.len() || !src.is_char_boundary(bang.start) || !src[bang.start..].starts_with('!') {
        return;
    }
    let start = src[..bang.start]
        .char_indices()
        .rev()
        .take_while(|&(_, c)| c.is_alphanumeric() || c == '_' || c == ':')
        .last()
        .map_or(bang.start, |(i, _)| i);
    let name = src[start..bang.start].rsplit("::").next().unwrap_or("");
    if name.is_empty() {
        return;
    }
    let open = bang.end;
    let Some(close) = matching_close(src, open) else { return };
    let inner = &src[open + 1..close];
    let whole = Span::new(start, close + 1);
    let args = split_args(inner);
    let (target, needs_fmt) = match name {
        "println" | "print" => ("print", true),
        "panic" => ("panic", true),
        "assert" => ("assert", false),
        "assert_eq" => ("assert_eq", false),
        "assert_ne" if args.len() >= 2 => {
            add(Auto, whole, &format!("assert({} != {})", args[0].trim(), args[1].trim()), "Om has no macros".into());
            return;
        }
        "todo" | "unimplemented" => {
            add(Auto, whole, "panic(\"not yet implemented\")", "Om has no macros".into());
            return;
        }
        "unreachable" => {
            add(Auto, whole, "panic(\"entered unreachable code\")", "Om has no macros".into());
            return;
        }
        _ => {
            // Drop the `!` and hope the function exists.
            add(Maybe, bang, "", "call it as a function".into());
            return;
        }
    };
    if !needs_fmt {
        // assert!(cond) / assert!(cond, "msg {}", x)
        let mut out: Vec<String> =
            args.iter().take(if name == "assert" { 1 } else { 2 }).map(|a| a.trim().to_string()).collect();
        let rest = &args[out.len().min(args.len())..];
        if !rest.is_empty() {
            match format_string(rest) {
                Some((s, _)) => out.push(s),
                None => return,
            }
        }
        add(Auto, whole, &format!("{target}({})", out.join(", ")), "Om has no macros".into());
        return;
    }
    if args.is_empty() || (args.len() == 1 && args[0].trim().is_empty()) {
        let text = if target == "print" { "print(\"\")" } else { "panic()" };
        add(Auto, whole, text, "Om has no macros".into());
        return;
    }
    if let Some((s, exact)) = format_string(&args) {
        let app = if exact { Auto } else { Maybe };
        add(app, whole, &format!("{target}({s})"), "Om has no macros; `{x}` interpolates".into());
    }
}

/// Turn Rust format arguments (`"a {} b {:?}", x, y`) into one Om interpolated
/// string (`"a {x} b {y}"`). The bool is false when formatting options were dropped.
fn format_string(args: &[String]) -> Option<(String, bool)> {
    let fmt = args.first()?.trim();
    let body = fmt.strip_prefix('"')?.strip_suffix('"')?;
    let rest: Vec<&str> = args[1..].iter().map(|a| a.trim()).collect();
    if rest.iter().any(|a| a.contains(['"', '{', '}', '=']) || a.is_empty()) {
        return None;
    }
    let mut out = String::from("\"");
    let mut exact = true;
    let mut next = 0;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push_str("{{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push_str("}}");
            }
            '{' => {
                let mut spec = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    spec.push(c);
                }
                let (arg, opts) = spec.split_once(':').unwrap_or((&spec, ""));
                if !opts.is_empty() && !matches!(opts, "?" | "#?") {
                    exact = false;
                }
                let expr = if arg.is_empty() {
                    let e = rest.get(next)?;
                    next += 1;
                    e.to_string()
                } else if let Ok(i) = arg.parse::<usize>() {
                    rest.get(i)?.to_string()
                } else {
                    arg.to_string()
                };
                out.push('{');
                out.push_str(&expr);
                out.push('}');
            }
            '\\' => {
                out.push(c);
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    if next < rest.len() && !body.contains("{0") {
        return None;
    }
    Some((out, exact))
}

/// Index of the bracket that closes the one at `open`, on any line.
fn matching_close(src: &str, open: usize) -> Option<usize> {
    let b = src.as_bytes();
    let (o, c) = match b.get(open)? {
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        b'{' => (b'{', b'}'),
        _ => return None,
    };
    let mut depth = 0;
    let mut in_str = false;
    let mut i = open;
    while i < b.len() {
        let ch = b[i];
        if in_str {
            if ch == b'\\' {
                i += 1;
            } else if ch == b'"' {
                in_str = false;
            }
        } else if ch == b'"' {
            in_str = true;
        } else if ch == o {
            depth += 1;
        } else if ch == c {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Split call arguments at top-level commas.
fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for c in s.chars() {
        if in_str {
            cur.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                cur.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur);
    }
    out
}

/// `span` widened to its whole line (and newline) when nothing else is on it.
fn whole_line(src: &str, span: Span) -> Span {
    let ls = src[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let le = src[span.end..].find('\n').map_or(src.len(), |i| span.end + i);
    if src[ls..span.start].trim().is_empty() && src[span.end..le].trim().is_empty() {
        Span::new(ls, (le + 1).min(src.len()))
    } else {
        span
    }
}

/// `x + 1.5` with `x: i64` -> convert the integer side to the float type.
fn mismatched_operands(
    cx: &Context,
    span: Span,
    detail: &str,
    add: &mut impl FnMut(Applicability, Span, &str, String),
) {
    let mut words = detail.split_whitespace();
    let (Some(lt), Some(_op), Some(rt)) = (words.next(), words.next(), words.next()) else { return };
    let floaty = |t: &str| t == "f64" || t == "f32";
    let Some((lhs, rhs)) = find_binary(cx.program, span) else { return };
    let (side, target) = match (floaty(lt), floaty(rt)) {
        (false, true) => (lhs, rt),
        (true, false) => (rhs, lt),
        _ => (rhs, lt),
    };
    let text = cx.text(side);
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit() || c == '_') && floaty(target) {
        add(Applicability::Auto, Span::new(side.end, side.end), ".0", "write a float literal".into());
        return;
    }
    let simple = text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    let new = if simple { format!("{text} as {target}") } else { format!("({text}) as {target}") };
    add(Applicability::Maybe, side, &new, format!("convert with `as {target}`"));
}

fn missing_trait_method(
    cx: &Context,
    span: Span,
    method: &str,
    tn: &str,
    add: &mut impl FnMut(Applicability, Span, &str, String),
) {
    let Some(t) = prelude().traits.iter().chain(&cx.program.traits).find(|t| t.name == tn) else { return };
    let Some(m) = t.methods.iter().find(|m| m.name == method) else { return };
    let Some(imp) = cx.program.impls.iter().find(|i| i.ty.span == span) else { return };
    let close = imp.span.end.saturating_sub(1);
    if cx.src.as_bytes().get(close) != Some(&b'}') {
        return;
    }
    let sig = crate::doc::signature(m);
    let ret_default = if m.ret.is_some() { "panic(\"TODO\")" } else { "" };
    let indent = if imp.methods.is_empty() { "    " } else { "" };
    let line_start = cx.src[..close].rfind('\n').map_or(0, |i| i + 1);
    let at = if cx.src[line_start..close].trim().is_empty() { line_start } else { close };
    let nl = if at == close { " " } else { "" };
    let text = format!("{nl}{indent}{sig} {{ {ret_default} }}\n");
    add(Applicability::Placeholder, Span::new(at, at), &text, format!("add `{method}`"));
}

// ---------- program queries ----------

/// The function or method whose body contains `pos`.
pub fn enclosing_fn(p: &Program, pos: usize) -> Option<&Function> {
    p.functions
        .iter()
        .chain(p.impls.iter().flat_map(|i| &i.methods))
        .chain(p.traits.iter().flat_map(|t| &t.methods))
        .filter(|f| f.span.start <= pos && pos <= f.span.end)
        .min_by_key(|f| f.span.end - f.span.start)
}

fn find_fn<'p>(p: &'p Program, name: &str, pos: usize) -> Option<&'p Function> {
    enclosing_fn(p, pos).filter(|f| f.name == name)
}

/// Names bound before `pos` in the function around it: parameters, `let`s, `for` variables and arm bindings.
fn locals_before(p: &Program, pos: usize) -> BTreeSet<String> {
    struct V(BTreeSet<String>, usize);
    impl Visitor for V {
        fn pattern(&mut self, p: &Pattern) {
            if let PatKind::Bind(n, _) = &p.kind {
                if p.span.start < self.1 {
                    self.0.insert(n.clone());
                }
            }
        }
        fn stmt(&mut self, s: &Stmt) {
            if let Stmt::For { var, span, .. } = s {
                if span.start < self.1 {
                    self.0.insert(var.clone());
                }
            }
        }
    }
    let mut v = V(BTreeSet::new(), pos);
    if let Some(f) = enclosing_fn(p, pos) {
        v.0.extend(f.params.iter().map(|p| p.name.clone()));
        visit::walk_function(&mut v, f);
    }
    v.0
}

/// The span of the immutable `let` pattern that binds `name` most recently before `pos`.
fn binding_site(p: &Program, name: &str, pos: usize) -> Option<Span> {
    struct V<'n>(&'n str, usize, Option<Span>);
    impl Visitor for V<'_> {
        fn stmt(&mut self, s: &Stmt) {
            if let Stmt::Let { pat, .. } = s {
                if let PatKind::Bind(n, false) = &pat.kind {
                    if n == self.0 && pat.span.start < self.1 {
                        self.2 = Some(pat.span);
                    }
                }
            }
        }
    }
    let f = enclosing_fn(p, pos)?;
    let mut v = V(name, pos, None);
    visit::walk_function(&mut v, f);
    v.2
}

/// Operand spans of the binary expression spanning exactly `span`.
fn find_binary(p: &Program, span: Span) -> Option<(Span, Span)> {
    struct V(Span, Option<(Span, Span)>);
    impl Visitor for V {
        fn expr(&mut self, e: &Expr) {
            if let ExprKind::Binary(_, a, b) = &e.kind {
                if e.span == self.0 {
                    self.1 = Some((a.span, b.span));
                }
            }
        }
    }
    let mut v = V(span, None);
    visit::walk_function(&mut v, enclosing_fn(p, span.start)?);
    v.1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(src: &str) -> String {
        crate::fix::apply_all(src, false).0
    }

    #[test]
    fn distance_ignores_case_and_underscores() {
        assert_eq!(distance("isEmpty", "is_empty"), 0);
        assert_eq!(distance("cout", "count"), 1);
        assert_eq!(similar("cout", &vec!["count".to_string(), "x".to_string()]), vec!["count"]);
    }

    #[test]
    fn rewrites_println_with_format_args() {
        assert_eq!(
            fixed("fn main() {\n    let x = 1\n    println!(\"x = {} and {:?}\", x, x + 1);\n}\n"),
            "fn main() {\n    let x = 1\n    print(\"x = {x} and {x + 1}\");\n}\n"
        );
        assert_eq!(fixed("fn main() { println!(\"hi\") }"), "fn main() { print(\"hi\") }");
        assert_eq!(fixed("fn main() { println!() }"), "fn main() { print(\"\") }");
    }

    #[test]
    fn removes_rust_only_syntax() {
        let src = "use std::io;\n#[derive(Debug, Clone)]\npub struct P { x: i64 }\npub fn main() { let s: String = \"a\".to_string(); print(s) }\n";
        assert_eq!(fixed(src), "struct P { x: i64 }\nfn main() { let s: str = \"a\"; print(s) }\n");
    }

    #[test]
    fn converts_angle_generics_and_ref_types() {
        let src = "fn first(x: Option<i64>) -> i64 { x.unwrap_or(0) }\nstruct S { name: &str }\nfn main() { print(first(None)) }\n";
        assert_eq!(fixed(src), "fn first(x: Option[i64]) -> i64 { x.unwrap_or(0) }\nstruct S { name: str }\nfn main() { print(first(None)) }\n");
    }

    #[test]
    fn adds_mut_and_reference_markers() {
        let src = "fn bump(x: &mut i64) { x += 1 }\nfn main() {\n    let n = 1\n    n = 2\n    bump(n)\n}\n";
        assert_eq!(
            fixed(src),
            "fn bump(x: &mut i64) { x += 1 }\nfn main() {\n    let mut n = 1\n    n = 2\n    bump(&mut n)\n}\n"
        );
    }

    #[test]
    fn suggests_close_names() {
        let src = "fn main() {\n    let count = 3\n    print(cout)\n}\n";
        let d = crate::check(src);
        assert_eq!(d[0].code, "E0200");
        assert_eq!(d[0].fixes[0].text, "count");
        assert_eq!(d[0].fixes[0].applicability, Applicability::Maybe);
        let d = crate::check("fn main() { let x: Option[i64] = None; print(x.is_sone()) }");
        assert_eq!(d[0].code, "E0205");
        assert_eq!(d[0].fixes[0].text, "is_some");
    }

    #[test]
    fn float_literal_fixes() {
        assert_eq!(fixed("fn main() { let x: f64 = 1; print(x) }"), "fn main() { let x: f64 = 1.0; print(x) }");
        assert_eq!(fixed("fn main() { let x = 1.5; print(x * 2) }"), "fn main() { let x = 1.5; print(x * 2.0) }");
    }

    #[test]
    fn rust_keywords_get_their_own_code() {
        let d = crate::check("pub fn main() {}");
        assert_eq!(d[0].code, "E0109");
        assert!(d[0].message.contains("Om has no `pub`"), "{}", d[0].message);
    }
}
