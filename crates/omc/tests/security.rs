//! Hostile input and memory-safety checks.
//!
//! The compiler must never crash, hang or run code at build time, whatever the
//! source says; and the programs it builds must stop with a panic (exit 101)
//! instead of reading or writing memory they do not own.

use omc::{fix, fmt, render, TempDir};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// A small deterministic random generator (xorshift64*), so failures reproduce.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Snippets that tend to confuse lexers and parsers.
const NASTY: &[&str] = &[
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "\"",
    "'",
    "\\",
    "<",
    ">",
    "::",
    "!",
    "#[",
    "#![",
    "fn",
    "let",
    "mut",
    "match",
    "=>",
    "->",
    "|>",
    "..=",
    "?",
    "&mut",
    "&",
    "*",
    "0xFFFFFFFFFFFFFFFFFFFF",
    "1e999",
    "'a",
    "'outer:",
    "/*",
    "*/",
    "//",
    "\n",
    "\u{0}",
    "é",
    "\u{202e}",
    "{{",
    "}}",
    "\"{",
    "}\"",
    "uses",
    "intrinsic::ptr_alloc(1)",
    "Self",
    "self",
    "impl",
    "trait",
    "enum",
    "struct",
    "const",
    "defer",
    "break",
    "return",
    "loop {}",
    ";",
    ",",
    "=",
    "+",
    "-",
    "%",
    "<<",
    ">>",
    "as",
    "Vec[",
    "Box::new(",
    "String::from(",
    "[0; 99999999999]",
    ".0.0.0",
    "\t",
    "\r\n",
];

fn char_boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn mutate(src: &str, rng: &mut Rng) -> String {
    let mut s = src.to_string();
    for _ in 0..1 + rng.below(4) {
        let at = char_boundary(&s, rng.below(s.len() + 1));
        match rng.below(4) {
            0 => s.insert_str(at, NASTY[rng.below(NASTY.len())]),
            1 => {
                let end = char_boundary(&s, at + rng.below(40));
                s.replace_range(at..end, "");
            }
            2 => s.truncate(at),
            _ => {
                // Duplicate a slice, which nests or repeats constructs.
                let end = char_boundary(&s, at + rng.below(200));
                let piece = s[at..end].to_string();
                s.insert_str(at, &piece);
            }
        }
    }
    s
}

/// Everything a tool does with an untrusted file, none of which may panic.
fn exercise(src: &str) {
    let diags = omc::check(src);
    for d in &diags {
        assert!(d.span.is_none_or(|s| s.start <= s.end && s.end <= src.len()), "span outside the file: {d:?}");
        let _ = render::human(d, "fuzz.om", src);
        let _ = render::brief(d, "fuzz.om", src);
        let _ = render::json(d, "fuzz.om", src);
    }
    let formatted = fmt::format(src);
    assert_eq!(fmt::format(&formatted), formatted, "fmt is not idempotent");
    let (fixed, _, _) = fix::apply_all(src, true);
    let _ = omc::check(&fixed);
}

fn examples() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [root.join("examples"), root.join("crates/omc/tests")] {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "om") {
                out.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn mutated_programs_never_crash_the_compiler() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut n = 0;
    for (name, src) in examples() {
        for _ in 0..12 {
            let m = mutate(&src, &mut rng);
            let start = Instant::now();
            let r = catch_unwind(AssertUnwindSafe(|| exercise(&m)));
            assert!(r.is_ok(), "the compiler crashed on a mutant of {name}:\n{m}");
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "the compiler took too long on a mutant of {name}:\n{m}"
            );
            n += 1;
        }
    }
    assert!(n > 50, "only {n} mutants");
}

#[test]
fn pathological_sources_fail_fast_with_a_code() {
    let deep_parens = format!("fn main() {{ print({}1{}) }}", "(".repeat(100_000), ")".repeat(100_000));
    let deep_blocks = format!("fn main() {}{}", "{".repeat(100_000), "}".repeat(100_000));
    let deep_type = format!("fn f(x: {}i64{}) {{}}\nfn main() {{}}", "Option[".repeat(50_000), "]".repeat(50_000));
    let long_chain = format!("fn main() {{ print(1{}) }}", " + 1".repeat(200_000));
    let unary = format!("fn main() {{ print({}1) }}", "-".repeat(100_000));
    let methods = format!("fn main() {{ print(1{}) }}", ".abs()".repeat(100_000));
    let interp = format!("fn main() {{ print(\"{{{}1{}}}\") }}", "(".repeat(100_000), ")".repeat(100_000));
    let errors =
        (0..5_000).map(|i| format!("fn f{i}() {{ let x: i64 = \"a\" }}\n")).collect::<String>() + "fn main() {}\n";
    let wide = (0..40)
        .map(|i| {
            if i == 0 {
                "struct A0 { a: i64, b: i64 }\n".to_string()
            } else {
                format!("struct A{i} {{ a: A{}, b: A{} }}\n", i - 1, i - 1)
            }
        })
        .collect::<String>()
        + "fn main() { let x: A39 = A39::default() }\n";
    let poly = "fn grow[T](x: T, n: i64) { if n > 0 { grow(Box::new(x), n - 1) } }\nfn main() { grow(1, 3) }\n";
    let generic_bomb =
        "fn f[T](x: T, n: i64) -> i64 { if n == 0 { 0 } else { f((x, x), n - 1) } }\nfn main() { print(f(1, 64)) }\n";
    let cases: Vec<(&str, String, &str)> = vec![
        ("deep parens", deep_parens, "E0108"),
        ("deep blocks", deep_blocks, "E0108"),
        ("deep type", deep_type, "E0108"),
        ("long chain", long_chain, "E0108"),
        ("unary", unary, "E0108"),
        ("method chain", methods, "E0108"),
        ("interpolation", interp, "E0108"),
        ("many errors", errors, "E0301"),
        ("wide struct", wide, ""),
        ("generic bomb", generic_bomb.to_string(), "E0312"),
        ("polymorphic recursion", poly.to_string(), "E0312"),
    ];
    for (what, src, code) in cases {
        let start = Instant::now();
        let d =
            catch_unwind(AssertUnwindSafe(|| omc::check(&src))).unwrap_or_else(|_| panic!("{what}: compiler crashed"));
        assert!(start.elapsed() < Duration::from_secs(30), "{what}: took {:?}", start.elapsed());
        assert!(d.iter().any(|d| d.is_error()), "{what}: no error");
        assert!(d.len() <= 400, "{what}: {} diagnostics", d.len());
        if !code.is_empty() {
            assert!(d.iter().any(|x| x.code == code), "{what}: expected {code}, got {:?}", d.first());
        }
        for x in &d {
            assert!(x.span.is_none_or(|s| s.end <= src.len()), "{what}: span outside the file: {x:?}");
        }
    }
}

#[test]
fn user_code_cannot_reach_raw_memory() {
    for src in [
        "fn main() { let p: Ptr[i64] = intrinsic::ptr_alloc(1) }",
        "fn main() { intrinsic::ptr_free(intrinsic::ptr_null()) }",
        "fn main() { let v = [1, 2] print(v.buf) }",
        "fn main() { let v: Vec[i64] = Vec { buf: intrinsic::ptr_null(), len: 9, cap: 9 } }",
        "fn main() { let s = \"a\".to_string() print(s.vec.len) }",
    ] {
        let d = omc::check(src);
        assert!(
            d.iter().any(|d| d.code == "E0209" || d.code == "E1004"),
            "`{src}` should be rejected as internal: {d:?}"
        );
    }
}

/// Build `src`, run it, and return (stdout, stderr, exit code).
fn run(src: &str) -> (String, String, i32) {
    let dir = TempDir::new().unwrap();
    let exe = dir.path().join("prog");
    omc::build(src, &exe).unwrap_or_else(|d| panic!("{}", d.render("prog", src)));
    let out = omc::spawn(Command::new(&exe).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()))
        .and_then(|c| c.wait_with_output())
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn programs_panic_instead_of_corrupting_memory() {
    let cases = [
        ("fn main() { let x: i64 = 9223372036854775807 print(x + 1) }", "attempt to add with overflow"),
        ("fn main() { let x: u8 = 0 print(x - 1) }", "attempt to subtract with overflow"),
        ("fn main() { let x: i32 = 65536 print(x * x) }", "attempt to multiply with overflow"),
        ("fn main() { let x: i64 = -9223372036854775807 - 1 print(-x) }", "attempt to negate with overflow"),
        ("fn main() { let x: i64 = -9223372036854775807 - 1 print(x / -1) }", "attempt to divide with overflow"),
        ("fn main() { let z: i64 = 0 print(1 / z) }", "attempt to divide by zero"),
        ("fn main() { let z: u32 = 0 print(1 % z) }", "remainder with a divisor of zero"),
        ("fn main() { let s: u32 = 40 print(1u32 << s) }", "attempt to shift left with overflow"),
        ("fn main() { let v = [1, 2, 3] let i: usize = 3 print(v[i]) }", "index out of bounds"),
        ("fn main() { let v: Vec[i64] = Vec::with_capacity(2305843009213693952) print(v.len()) }", "capacity overflow"),
        ("fn main() { let v = [7; 2305843009213693953] print(v.len()) }", "capacity overflow"),
        ("fn main() { print(\"ab\".repeat(4611686018427387904).len()) }", "capacity overflow"),
        ("fn main() { print(\"héllo\".slice(0, 2)) }", "not on a char boundary"),
        ("fn main() { print(\"hello\".slice(3, 100)) }", "out of bounds"),
        ("fn main() { let mut v = [1] v.remove(4) print(v) }", "removal index"),
    ];
    for (src, want) in cases {
        let (_, err, code) = run(src);
        assert_eq!(code, 101, "`{src}` exited with {code}: {err}");
        assert!(err.contains(want), "`{src}`: expected a panic containing `{want}`, got: {err}");
    }
}

#[test]
fn printed_text_is_data_not_a_format_string() {
    let (out, _, code) = run("fn main() { let s = \"%s%n%x%p\" print(s) print(\"{s} {{}}\") }");
    assert_eq!((out.as_str(), code), ("%s%n%x%p\n%s%n%x%p {}\n", 0));
}

#[test]
fn user_functions_do_not_replace_runtime_or_libc_symbols() {
    let src = "fn malloc(n: i64) -> i64 { n + 1 }\nfn free(n: i64) -> i64 { n }\n\
               fn om_rt_alloc(n: i64) -> i64 { n * 2 }\nfn exit(n: i64) -> i64 { n }\n\
               fn main() { let mut s = \"a\".to_string() s.push_str(\"b\") print(\"{malloc(1)} {free(2)} {om_rt_alloc(3)} {exit(4)} {s}\") }\n";
    let (out, _, code) = run(src);
    assert_eq!((out.as_str(), code), ("2 2 6 4 ab\n", 0));
}

#[test]
fn compiling_never_runs_user_code() {
    // Om has no macros, build scripts, `const fn` evaluation or compile-time
    // includes: a program that would loop forever or touch files only does so
    // when it is run.
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("ran");
    let src = format!(
        "fn main() {{ loop {{}} }}\nconst PATH: str = \"{}\"\n",
        marker.display().to_string().replace('\\', "\\\\")
    );
    let start = Instant::now();
    let d = omc::check(&src);
    assert!(d.is_empty(), "{d:?}");
    omc::build(&src, &dir.path().join("prog")).unwrap();
    assert!(start.elapsed() < Duration::from_secs(30));
    assert!(!marker.exists());
}
