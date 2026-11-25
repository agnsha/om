//! `om test`: run every `fn test_*()` in a file, each in its own process.
//!
//! Output is built for agents: passing tests are a count, failures carry only
//! the panic message (`assert_eq` prints left and right), and results of
//! passing tests are cached by a SHA-256 of the compiler version, the runtime
//! and the source, so an unchanged test never runs twice.

use crate::{json, lints, Diagnostic, TempDir};
use om_parser::ast::Program;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Most bytes kept from a failing test's output.
const MAX_OUTPUT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Cached,
    /// Exit status (101 for a panic) and the end of stderr.
    Fail(i32, String),
    Timeout,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub outcome: Outcome,
}

pub struct Options {
    pub use_cache: bool,
    pub timeout: Duration,
}

/// Names of the test functions in `p`: `fn test_*()` with no parameters.
pub fn tests_in(p: &Program) -> Vec<String> {
    p.functions.iter().filter(|f| lints::is_test(f) && f.params.is_empty()).map(|f| f.name.clone()).collect()
}

/// Where passing results are cached: `$XDG_CACHE_HOME/om/tests` or `~/.cache/om/tests`.
fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dir = base.join("om").join("tests");
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(&dir).ok()?;
    Some(dir)
}

fn cache_key(src: &str, test: &str) -> String {
    let cc = std::env::var("CC").unwrap_or_default();
    let input = format!(
        "om {}\0{}\0{cc}\0{test}\0{src}",
        env!("CARGO_PKG_VERSION"),
        crate::sha256::hex(om_codegen::RUNTIME_C.as_bytes())
    );
    crate::sha256::hex(input.as_bytes())
}

/// Compile and run every test in `src`. Compile errors come back as diagnostics.
pub fn run(src: &str, opts: &Options) -> Result<Vec<TestResult>, Vec<Diagnostic>> {
    let analysis = crate::analyze(src, false);
    if analysis.diagnostics.iter().any(Diagnostic::is_error) {
        return Err(analysis.diagnostics);
    }
    let names = tests_in(&analysis.program);
    let cache = if opts.use_cache { cache_dir() } else { None };
    let dir = TempDir::new().map_err(|e| vec![Diagnostic::error(format!("creating temp dir: {e}"), None)])?;
    let mut results = Vec::new();
    let mut to_run = Vec::new();
    for name in names {
        let key = cache_key(src, &name);
        if cache.as_ref().is_some_and(|c| c.join(&key).is_file()) {
            results.push(TestResult { name, outcome: Outcome::Cached });
            continue;
        }
        let exe = dir.path().join(&name);
        build_test(&analysis.program, &name, &exe).map_err(|d| vec![d])?;
        to_run.push((name, exe, key));
    }
    // Run in parallel, a few at a time.
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).min(8);
    let outcomes: Vec<(String, Outcome, String)> = std::thread::scope(|s| {
        let mut out = Vec::new();
        for chunk in to_run.chunks(workers) {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(name, exe, key)| {
                    let dir = dir.path();
                    s.spawn(move || (name.clone(), run_one(exe, dir, opts.timeout), key.clone()))
                })
                .collect();
            out.extend(handles.into_iter().map(|h| h.join().expect("test runner thread")));
        }
        out
    });
    for (name, outcome, key) in outcomes {
        if outcome == Outcome::Pass {
            if let Some(c) = &cache {
                let _ = std::fs::write(c.join(&key), b"");
            }
        }
        results.push(TestResult { name, outcome });
    }
    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}

/// Build `program` with a `main` that calls only `test`.
fn build_test(program: &Program, test: &str, exe: &Path) -> Result<(), Diagnostic> {
    let mut p = program.clone();
    p.functions.retain(|f| f.name != "main");
    let main = om_parser::parse(&format!("fn main() {{ {test}() }}"))
        .map_err(|e| Diagnostic::error(format!("internal error: {e}"), None))?;
    p.functions.extend(main.functions);
    let object = om_codegen::compile_to_object(&p).map_err(|e| Diagnostic::error(e.message, Some(e.span)))?;
    crate::link(&object, exe)
}

fn run_one(exe: &Path, dir: &Path, timeout: Duration) -> Outcome {
    let name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let err_path = dir.join(format!("{name}.stderr"));
    let Ok(err_file) = std::fs::File::create(&err_path) else {
        return Outcome::Fail(-1, "could not create a file for the test's output".into());
    };
    let child = crate::spawn(Command::new(exe).stdin(Stdio::null()).stdout(Stdio::null()).stderr(err_file));
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(-1, format!("could not start the test: {e}")),
    };
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Outcome::Timeout;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => return Outcome::Fail(-1, format!("waiting for the test: {e}")),
        }
    };
    if status.success() {
        return Outcome::Pass;
    }
    let bytes = std::fs::read(&err_path).unwrap_or_default();
    let tail = &bytes[bytes.len().saturating_sub(MAX_OUTPUT)..];
    let text = String::from_utf8_lossy(tail).trim_end().to_string();
    #[cfg(unix)]
    let code = std::os::unix::process::ExitStatusExt::signal(&status).map_or(status.code().unwrap_or(-1), |s| 128 + s);
    #[cfg(not(unix))]
    let code = status.code().unwrap_or(-1);
    Outcome::Fail(code, text)
}

/// Brief report: only failures, then one summary line.
pub fn report(results: &[TestResult], as_json: bool) -> String {
    let passed = results.iter().filter(|r| matches!(r.outcome, Outcome::Pass | Outcome::Cached)).count();
    let cached = results.iter().filter(|r| r.outcome == Outcome::Cached).count();
    let failed = results.len() - passed;
    let mut out = String::new();
    for r in results {
        let (status, code, msg) = match &r.outcome {
            Outcome::Fail(c, m) => ("fail", *c, m.as_str()),
            Outcome::Timeout => ("timeout", -1, "did not finish in time"),
            _ => continue,
        };
        if as_json {
            let o = json::Obj::new().str("test", &r.name).str("status", status).num("exit", code).str("output", msg);
            out.push_str(&o.finish());
        } else {
            out.push_str(&format!("FAIL {} (exit {code})\n  {}", r.name, msg.replace('\n', "\n  ")));
        }
        out.push('\n');
    }
    if as_json {
        let o = json::Obj::new().num("passed", passed as i64).num("failed", failed as i64).num("cached", cached as i64);
        out.push_str(&o.finish());
    } else if results.is_empty() {
        out.push_str("no tests: write `fn test_name() { assert(...) }`");
    } else {
        out.push_str(&format!("{passed} passed ({cached} cached), {failed} failed"));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_tests_in_isolation_and_reports_failures() {
        let src = "fn add(a: i64, b: i64) -> i64 { a + b }\n\
                   fn test_add() { assert_eq(add(2, 2), 4) }\n\
                   fn test_wrong() { assert_eq(add(2, 2), 5) }\n\
                   fn test_spin() { loop {} }\n\
                   fn main() { print(add(1, 1)) }\n";
        let opts = Options { use_cache: false, timeout: Duration::from_secs(2) };
        let r = run(src, &opts).unwrap();
        let names: Vec<_> = r.iter().map(|r| (r.name.as_str(), r.outcome.clone())).collect();
        assert_eq!(names[0], ("test_add", Outcome::Pass));
        assert_eq!(names[1].0, "test_spin");
        assert_eq!(names[1].1, Outcome::Timeout);
        match &names[2].1 {
            Outcome::Fail(101, msg) => assert!(msg.contains("left: 4") && msg.contains("right: 5"), "{msg}"),
            other => panic!("{other:?}"),
        }
        let text = report(&r, false);
        assert!(text.ends_with("1 passed (0 cached), 2 failed\n"), "{text}");
        assert!(report(&r, true).lines().last().unwrap().starts_with(r#"{"passed":1,"failed":2"#));
    }
}
