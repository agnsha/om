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

fn compile_error(src: &str) -> String {
    omc::compile(src).expect_err("program should be rejected").message
}

#[test]
fn rejects_bad_programs() {
    let cases = [
        ("fn main() { print(x) }", "cannot find variable `x`"),
        ("fn main() { let a = 1; a = 2 }", "cannot assign twice to `a`"),
        ("fn main() { let a = 1 + true }", "mismatched types: i64 and bool"),
        ("fn f() -> i64 { true } fn main() {}", "must return i64"),
        ("fn main() { if 1 { } }", "condition must be bool"),
        ("fn f(a: i64) {} fn main() { f(1, 2) }", "takes 1 argument(s) but 2 were given"),
        ("fn f() {} fn f() {} fn main() {}", "defined twice"),
        ("fn helper() {}", "no `fn main()`"),
        ("fn main() { let s: str = 5 }", "declared as str but its value is i64"),
        ("fn main() { let v = if true { 1 } else { false } }", "different types"),
    ];
    for (src, want) in cases {
        let got = compile_error(src);
        assert!(got.contains(want), "for `{src}`\n  expected error containing: {want}\n  got: {got}");
    }
}
