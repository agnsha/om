//! Compile every program in `examples/`, run it, and compare its output with
//! the matching `.expected` file. Also check that bad programs are rejected
//! with the right message.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build_and_run(src: &str, name: &str) -> String {
    let exe = std::env::temp_dir().join(format!("om-test-{name}-{}", std::process::id()));
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
    let exe = std::env::temp_dir().join(format!("om-test-exit-{}", std::process::id()));
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
    // 1e3 is not valid Om yet; it reads as `1` then a name, so check that separately.
    assert!(omc::compile(src).is_err());
    let src = src.replace("print(1e3 == 1000.0)", "print(-0.5)");
    assert_eq!(build_and_run(&src, "floats"), "0.30000000000000004\n6.28318\n0.25\n3.5\n3\n-2\n2\ntrue\n-0.5\n");
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
    omc::compile(src).expect_err("program should be rejected").message
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
        ("fn main() { let s = \"x{1}\" }", "only be used inside `print`"),
        ("fn f() -> i64 { defer { return 1 } 2 } fn main() {}", "not allowed inside `defer`"),
        ("fn main() { print(1 |> 2) }", "right side of `|>`"),
        ("enum E { A, B } fn main() { match E::A { E::A => 1 } }", "missing: E::B "),
        ("fn main() { let x = match 3 { 1 => 1 } }", "missing: _"),
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
    let exe = dir.join(name);
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
        ("struct A {} fn main() { let x = A {} == A {} }", "implement `Eq` for it"),
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
