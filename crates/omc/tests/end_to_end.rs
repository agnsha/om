//! Compile every program in `examples/`, run it, and compare its output with
//! the matching `.expected` file. Also check that bad programs are rejected
//! with the right message.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `.exe` on Windows, so the built program can be started.
const EXE: &str = std::env::consts::EXE_SUFFIX;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build_and_run(src: &str, name: &str) -> String {
    // Tests run in parallel: give every build its own file.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let exe = std::env::temp_dir().join(format!("om-test-{name}-{}-{n}{EXE}", std::process::id()));
    omc::build(src, &exe).unwrap_or_else(|d| panic!("{}", d.render(name, src)));
    let out = Command::new(&exe).output().expect("run compiled program");
    let _ = std::fs::remove_file(&exe);
    assert!(out.status.success(), "{name} exited with {}", out.status);
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn examples_produce_expected_output() {
    let dir = root().join("examples");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("om") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(&path).unwrap();
        let expected = std::fs::read_to_string(path.with_extension("expected"))
            .unwrap_or_else(|_| panic!("missing examples/{name}.expected"));
        assert_eq!(build_and_run(&src, &name), expected, "output of examples/{name}.om");
        count += 1;
    }
    assert!(count >= 4, "expected at least 4 examples, found {count}");
}

#[test]
fn main_return_value_is_exit_code() {
    let exe = std::env::temp_dir().join(format!("om-test-exit-{}{EXE}", std::process::id()));
    omc::build("fn main() -> i64 { 3 + 4 }", &exe).unwrap();
    let status = Command::new(&exe).status().unwrap();
    let _ = std::fs::remove_file(&exe);
    assert_eq!(status.code(), Some(7));
}

#[test]
fn language_features() {
    let src = r#"
        fn abs(x: i64) -> i64 { if x < 0 { -x } else { x } }
        fn sign(x: i64) -> str {
            if x < 0 { return "negative" }
            if x == 0 { "zero" } else { "positive" }
        }
        fn main() {
            print(abs(-42))
            print(sign(-1))
            print(sign(0))
            print(sign(9))
            print(17 / 5)
            print(17 % 5)
            print(2 + 3 * 4 - (1 + 1))
            print(true || false && false)
            print(!(1 >= 2))
            let total = { let a = 10; let b = 20; a + b }
            print(total)
        }
    "#;
    let out = build_and_run(src, "features");
    assert_eq!(out, "42\nnegative\nzero\npositive\n3\n2\n12\ntrue\ntrue\n30\n");
}

#[test]
fn defer_runs_in_reverse_on_every_exit() {
    let src = r#"
        fn find(limit: i64) -> i64 {
            defer print("outer done")
            let mut i = 0
            while i < limit {
                defer print("iteration {i}")
                if i == 2 {
                    defer print("found it")
                    return i
                }
                i = i + 1
            }
            -1
        }
        fn main() {
            print(find(5))
            print(find(1))
        }
    "#;
    let out = build_and_run(src, "defer");
    assert_eq!(
        out,
        // Like Zig and Swift, a deferred expression is evaluated when it runs, so it
        // sees `i` after the increment.
        "iteration 1\niteration 2\nfound it\niteration 2\nouter done\n2\n\
         iteration 1\nouter done\n-1\n"
    );
}

#[test]
fn named_default_pipe_and_interpolation() {
    let src = r#"
        fn greet(name: str, punct: str = "!", times: i64 = 1) {
            let mut i = 0
            while i < times {
                print("hi {name}{punct}")
                i = i + 1
            }
        }
        fn inc(x: i64, by: i64 = 1) -> i64 { x + by }
        fn main() {
            greet("ana")
            greet(times: 2, name: "bo", punct: "?")
            print(1 |> inc |> inc(by: 10))
            print("{-5} {true && false} {inc(1)}{inc(2)}")
        }
    "#;
    assert_eq!(build_and_run(src, "features2"), "hi ana!\nhi bo?\nhi bo?\n12\n-5 false 23\n");
}

#[test]
fn loops_break_continue_and_ranges() {
    let src = r#"
        fn main() {
            let mut sum = 0
            for i in 1..=10 {
                if i % 3 == 0 { continue }
                sum += i
            }
            print(sum)
            let mut n = 0
            while true {
                n += 1
                if n == 4 { break }
            }
            print(n)
            let first_square_over_50 = loop {
                n += 1
                if n * n > 50 { break n * n }
            }
            print(first_square_over_50)
            for i in 0..0 { print(i) }
            let mut x = 100
            x -= 1; x *= 2; x /= 3; x %= 50
            print(x)
        }
    "#;
    assert_eq!(build_and_run(src, "loops"), "37\n4\n64\n16\n");
}

#[test]
fn floats_and_casts() {
    let src = r#"
        const PI: f64 = 3.14159
        fn main() {
            print(0.1 + 0.2)
            print(PI * 2.0)
            print(1.0 / 4.0)
            print(7 as f64 / 2.0)
            print(3.99 as i64)
            print(-2.5 as i64)
            print(true as i64 + 1)
            print(2.0 > 1.5 && 1.0 == 1.0)
            print(1e3 == 1000.0)
        }
    "#;
    assert_eq!(build_and_run(src, "floats"), "0.30000000000000004\n6.28318\n0.25\n3.5\n3\n-2\n2\ntrue\ntrue\n");
}

#[test]
fn structs_methods_and_mut_refs() {
    let src = r#"
        struct Counter { count: i64, step: i64 }
        struct Pair { a: Counter, b: Counter }
        impl Counter {
            fn new(step: i64 = 1) -> Self { Self { count: 0, step } }
            fn tick(&mut self) { self.count += self.step }
            fn ticked(self) -> Counter { let mut c = self; c.tick(); c }
            fn get(&self) -> i64 { self.count }
        }
        fn swap(a: &mut i64, b: &mut i64) { let t = a; a = b; b = t }
        fn main() {
            let mut c = Counter::new()
            c.tick()
            c.tick()
            print(c.get())
            let mut p = Pair { a: Counter::new(step: 10), b: c }
            p.a.tick()
            p.b.tick()
            print(p)
            print(Counter::new(5).ticked().ticked().get())
            let mut x = 1
            let mut y = 2
            swap(&mut x, &mut y)
            print("{x} {y}")
            let copy = p.a
            p.a.tick()
            print("{copy.count} {p.a.count}")
        }
    "#;
    assert_eq!(
        build_and_run(src, "structs"),
        "2\nPair { a: Counter { count: 10, step: 10 }, b: Counter { count: 3, step: 1 } }\n10\n2 1\n10 20\n"
    );
}

#[test]
fn enums_and_match() {
    let src = r#"
        enum Token { Num(i64), Plus, Times, Neg(i64, bool) }
        fn value(t: Token) -> i64 {
            match t {
                Token::Num(n) if n > 100 => 100,
                Token::Num(n) => n,
                Token::Neg(n, true) => -n,
                Token::Neg(_, false) => 0,
                Token::Plus | Token::Times => -1,
            }
        }
        fn main() {
            print(value(Token::Num(7)))
            print(value(Token::Num(700)))
            print(value(Token::Neg(4, true)))
            print(value(Token::Neg(4, false)))
            print(value(Token::Times))
            print(Token::Neg(3, false))
            let t = Token::Plus
            print(t)
            let label = match 42 { 0 => "zero", 1..=9 => "digit", 10..100 => "two digits", _ => "big" }
            print(label)
            print(match true { true => 1, false => 0 })
        }
    "#;
    assert_eq!(build_and_run(src, "enums"), "7\n100\n-4\n0\n-1\nNeg(3, false)\nPlus\ntwo digits\n1\n");
}

fn compile_error(src: &str) -> String {
    match omc::compile(src) {
        Ok(_) => panic!("program should be rejected: {src}"),
        Err(e) => e.message,
    }
}

#[test]
fn rejects_bad_programs() {
    let cases = [
        ("fn main() { print(x) }", "cannot find variable `x`"),
        ("fn main() { let a = 1; a = 2 }", "cannot assign twice to `a`"),
        ("fn main() { let a = 1 + true }", "mismatched types: i64 + bool"),
        ("fn f() -> i64 { true } fn main() {}", "must return i64"),
        ("fn main() { if 1 { } }", "condition must be bool"),
        ("fn f(a: i64) {} fn main() { f(1, 2) }", "takes 1 argument(s) but 2 were given"),
        ("fn f() {} fn f() {} fn main() {}", "defined twice"),
        ("fn helper() {}", "no `fn main()`"),
        ("fn main() { let s: str = 5 }", "expected str, but the value is i64"),
        ("fn main() { let v = if true { 1 } else { false } }", "different types"),
        ("fn f(a: i64 = 1, b: i64) {} fn main() {}", "`b` needs a default"),
        ("fn f(a: i64 = true) {} fn main() {}", "default for `a` is bool"),
        ("fn f(a: i64 = 1 + 1) {} fn main() {}", "must be constants"),
        ("fn f(a: i64) {} fn main() { f(b: 1) }", "no parameter named `b`"),
        ("fn f(a: i64, b: i64) {} fn main() { f(a: 1, 2) }", "positional arguments must come before"),
        ("fn f(a: i64) {} fn main() { f(1, a: 2) }", "given twice"),
        ("fn f(a: i64, b: i64) {} fn main() { f(1) }", "missing argument `b`"),
        ("fn f() -> i64 { defer { return 1 } 2 } fn main() {}", "not allowed inside `defer`"),
        ("fn main() { print(1 |> 2) }", "right side of `|>`"),
        ("enum E { A, B } fn main() { match E::A { E::A => 1 } }", "missing: E::B "),
        ("fn main() { let x = match 3 { 1 => 1 } }", "missing: i64::MIN..=0"),
        ("fn main() { match true { true => 1 } }", "missing: false"),
        ("enum T { N(i64, bool) } fn main() { match T::N(1, true) { T::N(_, true) => 1 } }", "missing: T::N(_, false)"),
        ("struct P { x: i64 } fn main() { let p = P { x: 1 }; p.x = 2 }", "cannot assign twice to `p`"),
        ("struct P { x: i64 } fn main() { let p = P { } }", "missing field(s) in `P`: x"),
        ("struct P { x: i64 } fn main() { let p = P { x: 1 }; print(p.y) }", "has no field `y`"),
        ("struct P { x: i64 } impl P { fn f(&self) { self.x = 1 } } fn main() {}", "take `&mut self`"),
        ("fn f(a: &mut i64) {} fn main() { let x = 1; f(&mut x) }", "declare it with `let mut x`"),
        ("fn f(a: &mut i64) {} fn main() { let mut x = 1; f(x) }", "pass it as `&mut ...`"),
        ("fn f(a: &mut i64, b: &mut i64) {} fn main() { let mut x = 1; f(&mut x, &mut x) }", "as `&mut` twice"),
        ("fn main() { break }", "`break` outside of a loop"),
        ("fn main() { while true { break 1 } }", "only `loop` can break with a value"),
        ("fn main() { let x = 1 + 2.0 }", "mismatched types: i64 + f64"),
        ("struct A { b: B } struct B { a: A } fn main() {}", "contains itself"),
        ("enum E { A(i64) } fn main() { match E::A(1) { E::A(x, y) => 1 } }", "has 1 field(s), but the pattern has 2"),
        ("fn main() { let x = 1; x.foo() }", "i64 has no method named `foo`"),
    ];
    for (src, want) in cases {
        let got = compile_error(src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}

fn run_status(src: &str, name: &str) -> (String, String, i32) {
    let dir = std::env::temp_dir().join(format!("om-e2e-status-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join(format!("{name}{EXE}"));
    omc::build(src, &exe).expect("build should succeed");
    let out = std::process::Command::new(&exe).output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn generics_option_result_and_try() {
    let src = r#"
        struct Wrap[T] { v: T }
        impl[T] Wrap[T] { fn get(self) -> T { self.v } }
        enum List[T] { Nil, One(T) }
        fn id[T](x: T) -> T { x }
        fn half(n: i64) -> Result[i64, str] { if n % 2 == 1 { return Err("odd") } Ok(n / 2) }
        fn quarter(n: i64) -> Result[i64, str] { let h = half(n)?; half(h) }
        fn first_neg(a: i64, b: i64) -> Option[i64] { if a < 0 { return Some(a) } if b < 0 { Some(b) } else { None } }
        fn both(a: i64, b: i64) -> Option[i64] { let x = first_neg(a, 0)?; let y = first_neg(b, 0)?; Some(x + y) }
        fn main() {
            print(id(id(5)) + 1)
            print(Wrap { v: "w" }.get())
            let l: List[bool] = List::One(true)
            print(l)
            print(quarter(8))
            print(quarter(6))
            print(both(-1, -2))
            print(both(-1, 2))
            print(first_neg(3, -4).unwrap_or(0))
            print(half(3).is_err())
            print(half(4).ok())
            let none: Option[i64] = None
            print(none.unwrap_or(9))
        }
    "#;
    assert_eq!(
        build_and_run(src, "generics"),
        "6\nw\nOne(true)\nOk(2)\nErr(\"odd\")\nSome(-3)\nNone\n-4\ntrue\nSome(2)\n9\n"
    );
}

#[test]
fn tuples_patterns_if_let_and_while_let() {
    let src = r#"
        fn split(n: i64) -> (i64, i64) { (n / 10, n % 10) }
        fn main() {
            let (tens, ones) = split(47)
            print("{tens} {ones}")
            let t = (1, (false, "x"))
            print(t.1.1)
            let (a, (mut b, _)) = t
            b = !b
            print("{a} {b}")
            let maybe = Some(5)
            if let Some(v) = maybe { print(v) } else { print("none") }
            if let None = maybe { print("bad") } else { print("some") }
            let mut n: Option[i64] = Some(3)
            let mut total = 0
            while let Some(k) = n {
                total += k
                n = if k > 1 { Some(k - 1) } else { None }
            }
            print(total)
            print(match (1, true) { (0, _) => "zero", (_, true) => "yes", (_, false) => "no" })
        }
    "#;
    assert_eq!(build_and_run(src, "tuples"), "4 7\nx\n1 true\n5\nsome\n6\nyes\n");
}

#[test]
fn panic_and_assert_exit_101() {
    let (out, err, code) = run_status(r#"fn main() { print("before") panic("bad {1 + 1}") }"#, "panic");
    assert_eq!((out.as_str(), err.as_str(), code), ("before\n", "panic: bad 2\n", 101));
    let (_, err, code) = run_status("fn main() { let x: Option[i64] = None; print(x.unwrap()) }", "unwrap");
    assert_eq!((err.as_str(), code), ("panic: called `Option::unwrap()` on a `None` value\n", 101));
    let (_, err, code) = run_status("fn main() { assert(1 > 2) }", "assert");
    assert_eq!((err.as_str(), code), ("panic: assertion failed\n", 101));
    let (_, err, code) = run_status(r#"fn main() { assert(false, "must hold") }"#, "assert_msg");
    assert_eq!((err.as_str(), code), ("panic: must hold\n", 101));
    let (out, _, code) = run_status("fn main() { assert(1 < 2) print(1) }", "assert_ok");
    assert_eq!((out.as_str(), code), ("1\n", 0));
}

#[test]
fn rejects_bad_generic_programs() {
    let cases = [
        ("fn f() -> i64 { let a = Some(2)?; a } fn main() {}", "can only be used in a function that returns Option"),
        (
            "fn f() -> Result[i64, bool] { let a: Result[i64, str] = Ok(1); let b = a?; Ok(b) } fn main() {}",
            "cannot convert the error type str into bool",
        ),
        ("fn main() { let Some(x) = Some(1) }", "might not match"),
        ("fn main() { let x = None }", "cannot infer type `T`"),
        ("fn main() { let (a, b) = (1, 2, 3) }", "tuple of 2"),
        ("fn id[T](x: T) -> T { x } fn main() { let a: bool = id(1) }", "i64"),
        ("struct W[T] { v: T } fn main() { let w: W[i64] = W { v: true } }", "bool"),
    ];
    for (src, want) in cases {
        let got = compile_error(src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}

#[test]
fn traits_bounds_and_operators() {
    let src = r#"
        trait Speak {
            fn sound(&self) -> str
            fn speak(&self) { print("says {self.sound()}") }
        }
        struct Dog {}
        struct Cat { loud: bool }
        impl Speak for Dog { fn sound(&self) -> str { "woof" } }
        impl Speak for Cat {
            fn sound(&self) -> str { if self.loud { "MEOW" } else { "meow" } }
            fn speak(&self) { print("cat {self.sound()}") }
        }
        fn twice[T](x: T) where T: Speak { x.speak() x.speak() }
        struct M { v: i64 }
        impl Mul for M { fn mul(self, o: M) -> M { M { v: self.v * o.v } } }
        impl Eq for M { fn eq(&self, o: &M) -> bool { self.v == o.v } }
        impl Ord for M { fn cmp(&self, o: &M) -> Ordering { self.v.cmp(&o.v) } }
        fn biggest[T: Ord + Default](a: T, b: T) -> T { if a > b { a } else { b } }
        fn main() {
            Dog {}.speak()
            twice(Cat { loud: true })
            print(M { v: 3 } * M { v: 4 })
            print(M { v: 1 } <= M { v: 1 })
            print(M { v: 2 } >= M { v: 3 })
            print(M { v: 2 } != M { v: 3 })
            print(biggest(4, 8))
            print(M { v: 9 }.min(M { v: 5 }))
            print(3.cmp(&3))
            let d: f64 = f64::default()
            print(d)
        }
    "#;
    assert_eq!(
        build_and_run(src, "traits"),
        "says woof\ncat MEOW\ncat MEOW\nM { v: 12 }\ntrue\nfalse\ntrue\n8\nM { v: 5 }\nEqual\n0\n"
    );
}

#[test]
fn rejects_bad_trait_programs() {
    let cases = [
        ("trait S { fn f(&self) -> i64 } struct A {} impl S for A {} fn main() {}", "missing method `f`"),
        (
            "trait S { fn f(&self) -> i64 } struct A {} impl S for A { fn f(&self) -> bool { true } } fn main() {}",
            "expected `fn f(&self) -> i64`, found `fn f(&self) -> bool`",
        ),
        ("trait S { fn f(&self) } struct A {} impl S for A { fn f(&self) {} fn g(&self) {} } fn main() {}", "`g` is not a method of trait `S`"),
        ("struct A {} impl Shape for A {} fn main() {}", "cannot find trait `Shape`"),
        ("fn f[T](x: T) where U: Eq {} fn main() {}", "`U` is not a type parameter"),
        ("struct A {} fn big[T: Ord](a: T) {} fn main() { big(A {}) }", "A does not implement `Ord`"),
        // Structs, tuples and data-free enums compare field by field; enums with data need `Eq`.
        ("enum A { X(i64) } fn main() { let x = A::X(1) == A::X(1) }", "implement `Eq` for it"),
        ("struct A {} fn main() { A {}.max(A {}) }", "trait `Ord` has one, but A does not implement it"),
        ("impl i64 { fn f(self) {} } fn main() {}", "implement a trait for it"),
        (
            "struct W[T] { v: T } struct A {} impl[T: Eq] Eq for W[T] { fn eq(&self, o: &W[T]) -> bool { true } } fn main() { print(W { v: A {} } == W { v: A {} }) }",
            "not defined for W[A]",
        ),
    ];
    for (src, want) in cases {
        let got = compile_error(src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}

#[test]
fn integer_types_chars_bits_and_labels() {
    // Each line of the expected output was checked against what Rust prints.
    let dir = root().join("crates/omc/tests");
    let src = std::fs::read_to_string(dir.join("numbers_and_chars.om")).unwrap();
    let want = std::fs::read_to_string(dir.join("numbers_and_chars.expected")).unwrap();
    assert_eq!(build_and_run(&src, "numbers"), want);
}

#[test]
fn rejects_bad_number_programs() {
    let cases = [
        ("fn main() { let x: u8 = 256 }", "does not fit in u8"),
        ("fn main() { let x: i8 = -129 }", "does not fit in i8"),
        ("fn main() { let x: u8 = 1; let y = -x }", "cannot negate an unsigned u8"),
        ("fn main() { let a: u8 = 1; let b: i32 = 2; let c = a + b }", "mismatched types: u8 + i32"),
        ("fn main() { loop { break 'nope } }", "no enclosing loop is labelled `'nope`"),
        ("fn main() { 'a: loop { 'a: loop { break } } }", "already used by an enclosing loop"),
        ("fn main() { let x: u8 = 3; match x { 0..=100 => 1, 200..=255 => 2 } }", "missing: 101..=199"),
        ("fn main() { let x: u8 = 3; match x { 300 => 1, _ => 0 } }", "`300` does not fit in u8"),
        ("fn main() { let c = 'a' + 'b' }", "not defined for char"),
        ("fn main() { let x = 5 as char }", "cannot cast i64 to char"),
        ("fn main() { let x = 5xyz }", "invalid suffix `xyz`"),
        ("fn main() { let x = 300 as u8 }", "does not fit in u8"),
    ];
    for (src, want) in cases {
        let got = compile_error(src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}

#[test]
fn vec_moves_drops_and_borrows() {
    // Drop order and output were checked against the same program in Rust.
    let dir = root().join("crates/omc/tests");
    let src = std::fs::read_to_string(dir.join("ownership.om")).unwrap();
    let want = std::fs::read_to_string(dir.join("ownership.expected")).unwrap();
    assert_eq!(build_and_run(&src, "ownership"), want);
    let (_, err, code) = run_status("fn main() { let v = [1, 2]\n print(v[5]) }", "oob");
    assert_eq!(code, 101);
    assert!(err.contains("index out of bounds: the len is 2 but the index is 5"), "{err}");
}

#[test]
fn rejects_ownership_errors() {
    let n = "struct N { id: i64 } impl Drop for N { fn drop(&mut self) {} } fn take(n: N) {} ";
    let cases = [
        ("fn main() { let a = N { id: 1 }; take(a); take(a) }", "use of moved value `a`"),
        ("fn main() { let a = N { id: 1 }; for i in 0..3 { take(a) } }", "moved inside this loop"),
        (
            "fn main() { let mut v = [1]; let r = &v; v.push(2); print(r.len()) }",
            "cannot change `v` while `r` borrows it",
        ),
        ("fn main() { let v = [1]; v.push(2) }", "declare it with `let mut v`"),
        ("fn main() { let v = [1]; print(v.len) }", "the fields of `Vec` are private"),
        ("fn f(n: &N) -> N { n } fn main() {}", "cannot move out of `n`, which is borrowed"),
        (
            "fn main() { let a = N { id: 1 }; let b = if true { a } else { N { id: 2 } }; take(a) }",
            "use of moved value `a`",
        ),
        ("fn main() { let v = [N { id: 1 }]; let x = v[0] }", "cannot move parts out of this place"),
    ];
    for (src, want) in cases {
        let src = format!("{n}{src}");
        let got = compile_error(&src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}

#[test]
fn strings_and_for_loops() {
    // Output checked against the same program in Rust (with Rust spellings of the
    // non-Rust helpers such as `parse_int` and `slice`).
    let dir = root().join("crates/omc/tests");
    let src = std::fs::read_to_string(dir.join("strings.om")).unwrap();
    let want = std::fs::read_to_string(dir.join("strings.expected")).unwrap();
    assert_eq!(build_and_run(&src, "strings"), want);
}

#[test]
fn rejects_dangling_string_views() {
    let cases = [
        (
            "fn main() { let mut s = String::from(\" a \"); let t = s.trim(); s.push_str(\"x\"); print(t) }",
            "cannot change `s` while `t` borrows it",
        ),
        ("fn main() { let t: str = \"{1}\"; print(t) }", "points into a temporary String"),
        ("fn f() -> str { let s = String::from(\"a\"); s.as_str() } fn main() {}", "points into `s`, which is dropped"),
        (
            "fn main() { let mut s = String::from(\"ab\"); s.push_str(s.as_str()) }",
            "while also passing a view of its text",
        ),
        (
            "fn main() { let s = String::from(\"ab\"); let mut v: Vec[str] = Vec::new(); v.push(s.as_str()) }",
            "could keep a `str` view",
        ),
        ("fn main() { let s = String::from(\"a\"); let mut t = \"x\"; t = s.as_str() }", "cannot store a `str` view"),
        (
            "fn main() { let s = String::from(\"a,b\"); let p = s.split(\",\"); take(s) } fn take(s: String) {}",
            "cannot move `s` while `p` borrows it",
        ),
        ("fn main() { let mut v = [1]; for x in &mut v {} }", "not supported yet"),
        ("fn main() { let s = String { vec: Vec::new() } }", "has private fields"),
    ];
    for (src, want) in cases {
        let got = compile_error(src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}

#[test]
fn boxes_and_recursive_types() {
    // Output and drop order checked against the same program in Rust.
    let dir = root().join("crates/omc/tests");
    let src = std::fs::read_to_string(dir.join("boxes.om")).unwrap();
    let want = std::fs::read_to_string(dir.join("boxes.expected")).unwrap();
    assert_eq!(build_and_run(&src, "boxes"), want);
}

#[test]
fn float_printing_matches_rust() {
    // `print(x)` matches Rust's `{}`; floats inside other values match `{:?}`.
    let dir = root().join("crates/omc/tests");
    let src = std::fs::read_to_string(dir.join("floats.om")).unwrap();
    let want = std::fs::read_to_string(dir.join("floats.expected")).unwrap();
    assert_eq!(build_and_run(&src, "floats"), want);
}

#[test]
fn borrowed_list_loops_self_constructors_and_string_eq() {
    // `for s in v` over a `&Vec[String]` once freed each String twice.
    let dir = root().join("crates/omc/tests");
    let src = std::fs::read_to_string(dir.join("regressions.om")).unwrap();
    let want = std::fs::read_to_string(dir.join("regressions.expected")).unwrap();
    assert_eq!(build_and_run(&src, "regressions"), want);
    let got = compile_error("fn f(v: &Vec[i64]) { let mut w = v; w.push(1) } fn main() {}");
    assert!(got.contains("cannot change `w`: it is borrowed with `&`"), "{got}");
    let got = compile_error("fn f(v: &Vec[i64]) { v[0] = 2 } fn main() {}");
    assert!(got.contains("read-only") || got.contains("cannot"), "{got}");
}

/// Build `src` and run it with `input` on stdin; return stdout.
fn run_with_input(src: &str, name: &str, input: &[u8]) -> String {
    use std::io::Write;
    use std::process::Stdio;
    let exe = std::env::temp_dir().join(format!("om-test-{name}-{}{EXE}", std::process::id()));
    omc::build(src, &exe).unwrap_or_else(|d| panic!("{}", d.render(name, src)));
    let mut child =
        Command::new(&exe).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("run compiled program");
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    let _ = std::fs::remove_file(&exe);
    assert!(out.status.success(), "{name} exited with {}", out.status);
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn read_key_time_and_sleep() {
    let src = r#"
        fn main() {
            let t0 = time_ns()
            sleep_ms(5)
            print(time_ns() - t0 >= 5_000_000)
            print(t0 > 1_600_000_000_000_000_000)
            let mut keys = String::new()
            loop {
                let k = read_key()
                if k < 0 { break }
                keys.push_str("{k} ")
            }
            print(keys)
        }
    "#;
    assert_eq!(run_with_input(src, "keys", b"a\n\x1b"), "true\ntrue\n97 10 27 \n");
}

#[test]
fn string_escapes_include_unicode_and_hex() {
    let src = r#"fn main() { print("\u{1b}[1m|\x41\u{e9}\'|\x1b[0m".len()) }"#;
    assert_eq!(build_and_run(src, "escapes"), "14\n");
}

#[test]
fn hollow_bell_game_builds_and_quits() {
    let src = std::fs::read_to_string(root().join("games/hollow-bell/hollow_bell.om")).unwrap();
    // Start a seeded game, take a few steps, chant, quit, and leave the title screen.
    let out = run_with_input(&src, "hollow_bell", b"s42\nllllckaom\n\nQyxq");
    assert!(out.contains("T H E    H O L L O W    B E L L"), "{out}");
    assert!(out.contains("Depth 1/6"), "{out}");
    assert!(out.contains("Seed 42"), "{out}");
}

#[test]
fn conveniences_from_writing_games() {
    let src = r#"
        const WALL: i64 = 0
        const FLOOR: i64 = 1
        const NEG: i64 = -5
        enum T { Wall, Floor }
        struct P { x: i64, y: i64 }
        struct Named { p: P, name: String, t: T }
        fn tile(t: i64) -> str { match t { WALL => "wall", FLOOR | 7 => "floor", NEG => "neg", _ => "?" } }
        fn label(x: i64) -> str { if x > 0 { "pos" } else { "neg" } }
        fn shout(s: String) -> String { s + "!" }
        fn sum(v: &Vec[i64]) -> i64 { let mut t = 0; for x in v { t += *x }; t }
        fn main() {
            // Any integer type indexes a list, and sizes a `[x; n]`.
            let v = [10, 20, 30]
            let i: i64 = 2
            let j: i32 = 1
            print("{v[i]} {v[j]} {[7; i]} {[1; -2i64]}")
            // Unused values at the end of a loop body or an else-less `if` are dropped.
            let mut log = ["a".to_string(), "b".to_string()]
            for _ in 0..3 { log.pop() }
            if log.is_empty() { log.push("c"); log.len() }
            print(log)
            // `str` becomes `String` where a String is wanted.
            let mut s = String::new()
            s = "hello"
            let n = Named { p: P { x: 1, y: 2 }, name: "monk", t: T::Floor }
            let mode = if n.p.x > 0 { "{n.p.x} high" } else { "low" }
            let loud = shout("hey")
            print("{s} {n.name} {mode} {loud} {label(-1)}")
            // `==` without an `Eq` impl, field by field.
            let m = Named { p: P { x: 1, y: 2 }, name: "monk".to_string(), t: T::Floor }
            print("{n == m} {T::Wall == T::Floor} {(1, 'a') == (1, 'a')} {P { x: 1, y: 2 } != P { x: 1, y: 3 }}")
            // Constants as patterns; `*x` on a plain value is the value.
            print("{tile(0)} {tile(7)} {tile(-5)} {tile(9)} {sum(&v)}")
            // `Vec::new()` takes its element type from a later push.
            let mut ps = Vec::new()
            for k in 0..2 { ps.push(P { x: k, y: k }) }
            let mut later = Vec::new()
            let c = 'z'
            later.push(c)
            print("{ps} {later}")
            print("no newline", end: "")
            print("|", end: "\n")
            eprint("to stderr")
        }
    "#;
    let (out, errout, code) = run_status(src, "conveniences");
    assert_eq!(code, 0, "{errout}");
    assert_eq!(
        out,
        "30 20 [7, 7] []\n[\"c\"]\nhello monk 1 high hey! neg\ntrue false true true\nwall floor neg ? 60\n\
         [P { x: 0, y: 0 }, P { x: 1, y: 1 }] ['z']\nno newline|\n"
    );
    assert_eq!(errout, "to stderr\n");
}

#[test]
fn args_read_line_and_poll_key() {
    let src = r#"
        fn main() {
            print(args().len())
            loop {
                match read_line() {
                    Some(l) => print("[{l}]"),
                    None => break,
                }
            }
            print(poll_key())
            print(term_cols() > 0 && term_rows() > 0)
        }
    "#;
    assert_eq!(run_with_input(src, "lines", b"hello\r\nworld\n\nlast"), "1\n[hello]\n[world]\n[]\n[last]\n-1\ntrue\n");
}
