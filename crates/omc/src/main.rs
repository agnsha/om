//! `om`: the command line tool for the Om language.

use omc::{codes, doc, fix, fmt, guide, json, render, testing, tokens, Diagnostic, TempDir};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::time::{Duration, Instant, SystemTime};

const USAGE: &str = "\
om: the Om language compiler

Usage:
  om build <file.om> [-o <output>]    compile to a native executable
  om run <file.om> [args...]          compile and run
  om check <file.om> [--brief|--json] [--max-errors N]
                                      report every error, with codes and fixes
  om fix <file.om> [--maybe] [--dry-run]
                                      apply fixes marked auto (and maybe) until clean
  om explain <CODE>                   long form of an error code
  om doc [NAME] [--file <file.om>]    signatures and docs; no NAME lists everything
  om guide [--codes]                  the language card for agents
  om test <file.om> [--json] [--no-cache] [--timeout SECS]
                                      run fn test_*(), print only failures
  om fmt <file.om> [--check]          canonical layout
  om audit <file.om> [--json]         effects (io, panic) of every function
  om watch <file.om>                  re-check on every save
  om tokens <file>                    approximate LLM token count
  om --version                        print the version

Set OM_AGENT=1 to make --brief the default output of check, fix and watch.
";

/// Exit status for usage errors.
const USAGE_ERR: i32 = 2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match args.first().map(String::as_str) {
        Some("build") => cmd_build(rest),
        Some("run") => cmd_run(rest),
        Some("check") => cmd_check(rest),
        Some("fix") => cmd_fix(rest),
        Some("explain") => cmd_explain(rest),
        Some("doc") => cmd_doc(rest),
        Some("guide") => cmd_guide(rest),
        Some("test") => cmd_test(rest),
        Some("fmt") => cmd_fmt(rest),
        Some("audit") => cmd_audit(rest),
        Some("watch") => cmd_watch(rest),
        Some("tokens") => cmd_tokens(rest),
        Some("--version" | "-V") => {
            out(&format!("om {}\n", env!("CARGO_PKG_VERSION")));
            0
        }
        Some("--help" | "-h" | "help") => {
            out(USAGE);
            0
        }
        Some(other) => {
            eprint!("error: unknown command `{other}`\n\n{USAGE}");
            USAGE_ERR
        }
        None => {
            eprint!("{USAGE}");
            USAGE_ERR
        }
    };
    exit(code);
}

/// Write to stdout. A closed pipe (`om doc | head`) ends the program quietly
/// instead of panicking.
fn out(s: &str) {
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout.write_all(s.as_bytes()).and_then(|()| stdout.flush()) {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            exit(0);
        }
        eprintln!("error: writing output: {e}");
        exit(1);
    }
}

/// Parsed command line: positional arguments, flags, and `--key value` options.
struct Args {
    positional: Vec<String>,
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

impl Args {
    /// `valued` lists the options that take a value.
    fn parse(args: &[String], flags: &[&str], valued: &[&str]) -> Result<Args, String> {
        let mut a = Args { positional: Vec::new(), flags: Vec::new(), values: Vec::new() };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            if arg == "--" {
                a.positional.extend(it.by_ref().cloned());
                break;
            }
            if valued.contains(&arg.as_str()) {
                let v = it.next().ok_or_else(|| format!("`{arg}` needs a value"))?;
                a.values.push((arg.clone(), v.clone()));
            } else if flags.contains(&arg.as_str()) {
                a.flags.push(arg.clone());
            } else if arg.starts_with('-') && arg.len() > 1 {
                return Err(format!("unknown option `{arg}`"));
            } else {
                a.positional.push(arg.clone());
            }
        }
        Ok(a)
    }

    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }

    fn value(&self, key: &str) -> Option<&str> {
        self.values.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// The single input file.
    fn file(&self) -> Result<&str, String> {
        match self.positional.as_slice() {
            [f] => Ok(f),
            [] => Err("missing the input file".into()),
            _ => Err(format!("unexpected argument `{}`", self.positional[1])),
        }
    }
}

fn usage_error(msg: &str) -> i32 {
    eprintln!("error: {msg}\n\nRun `om help` for usage.");
    USAGE_ERR
}

fn load(path: &str) -> Result<String, i32> {
    omc::read_source(Path::new(path)).map_err(|e| {
        eprintln!("error: {e}");
        1
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Human,
    Brief,
    Json,
}

fn format_of(a: &Args) -> Format {
    if a.has("--json") {
        Format::Json
    } else if a.has("--brief") {
        Format::Brief
    } else if a.has("--human") {
        Format::Human
    } else if std::env::var("OM_AGENT").is_ok_and(|v| !v.is_empty() && v != "0") {
        Format::Brief
    } else {
        Format::Human
    }
}

fn render_all(diags: &[Diagnostic], file: &str, src: &str, format: Format, max: usize) -> String {
    let mut s = String::new();
    for d in diags.iter().take(max) {
        let line = match format {
            Format::Human => render::human(d, file, src),
            Format::Brief => render::brief(d, file, src),
            Format::Json => render::json(d, file, src),
        };
        s.push_str(&line);
        s.push('\n');
        if format == Format::Human {
            s.push('\n');
        }
    }
    let errors = diags.iter().filter(|d| d.is_error()).count();
    let warnings = diags.len() - errors;
    let fixable = render::auto_fix_count(diags);
    let hidden = diags.len().saturating_sub(max);
    match format {
        Format::Json => {
            let o = json::Obj::new()
                .str("file", file)
                .num("errors", errors as i64)
                .num("warnings", warnings as i64)
                .num("auto_fixable", fixable as i64)
                .num("not_shown", hidden as i64);
            s.push_str(&o.finish());
            s.push('\n');
        }
        _ if diags.is_empty() => s.push_str(&format!("ok: {file}\n")),
        _ => {
            s.push_str(&format!("{errors} error(s), {warnings} warning(s)"));
            if fixable > 0 {
                s.push_str(&format!(", {fixable} fixable with `om fix {file}`"));
            }
            if hidden > 0 {
                s.push_str(&format!(", {hidden} more not shown"));
            }
            s.push('\n');
        }
    }
    s
}

fn default_output(input: &str) -> PathBuf {
    let stem = Path::new(input).file_stem().and_then(|s| s.to_str()).unwrap_or("a");
    PathBuf::from(if cfg!(windows) { format!("{stem}.exe") } else { stem.to_string() })
}

fn build_to(input: &str, out_path: &Path) -> Result<(), i32> {
    let src = load(input)?;
    omc::build(&src, out_path).map_err(|d| {
        eprintln!("{}", d.render(input, &src));
        1
    })
}

fn cmd_build(args: &[String]) -> i32 {
    let a = match Args::parse(args, &[], &["-o"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let out_path = a.value("-o").map_or_else(|| default_output(input), PathBuf::from);
    match build_to(input, &out_path) {
        Ok(()) => 0,
        Err(c) => c,
    }
}

fn cmd_run(args: &[String]) -> i32 {
    let Some(input) = args.first() else {
        return usage_error("missing the input file");
    };
    // A fresh private directory: a predictable path in a shared temp dir
    // could be swapped for another program between building and running.
    let dir = match TempDir::new() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: creating temp dir: {e}");
            return 1;
        }
    };
    let exe = dir.path().join(default_output(input));
    if let Err(c) = build_to(input, &exe) {
        return c;
    }
    let status = omc::spawn(Command::new(&exe).args(&args[1..])).and_then(|mut c| c.wait());
    drop(dir);
    match status {
        Ok(s) => exit_code(s),
        Err(e) => {
            eprintln!("error: could not start the program: {e}");
            1
        }
    }
}

fn windows_exception(code: u32) -> String {
    let what = match code {
        0xC000_0005 => "access violation",
        0xC000_00FD => "stack overflow",
        0xC000_0094 => "integer division by zero",
        0xC000_0095 => "integer overflow",
        0xC000_001D => "illegal instruction",
        0xC000_013A => "stopped with Ctrl+C",
        _ => "exception",
    };
    format!("{what}, code {code:#010X}")
}

fn exit_code(s: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    if let Some(sig) = std::os::unix::process::ExitStatusExt::signal(&s) {
        eprintln!("error: the program was killed by signal {sig}");
        return 128 + sig;
    }
    match s.code() {
        // Windows reports crashes as exception codes such as 0xC0000005.
        Some(c) if cfg!(windows) && (c as u32) >= 0xC000_0000 => {
            eprintln!("error: the program crashed ({})", windows_exception(c as u32));
            1
        }
        Some(c) => c,
        None => 1,
    }
}

fn cmd_check(args: &[String]) -> i32 {
    let a = match Args::parse(args, &["--json", "--brief", "--human"], &["--max-errors"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let max = match a.value("--max-errors").map(str::parse::<usize>) {
        None => usize::MAX,
        Some(Ok(n)) => n,
        Some(Err(_)) => return usage_error("`--max-errors` needs a number"),
    };
    let src = match load(input) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let diags = omc::check(&src);
    let format = format_of(&a);
    out(&render_all(&diags, input, &src, format, max));
    i32::from(diags.iter().any(Diagnostic::is_error))
}

fn cmd_fix(args: &[String]) -> i32 {
    let a = match Args::parse(args, &["--maybe", "--dry-run", "--json", "--brief", "--human"], &[]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let src = match load(input) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let (fixed, applied, remaining) = fix::apply_all(&src, a.has("--maybe"));
    let format = format_of(&a);
    let mut report = String::new();
    for f in &applied {
        if format == Format::Json {
            report.push_str(&json::Obj::new().str("applied", f.code).str("label", &f.label).finish());
        } else {
            report.push_str(&format!("fixed {} {}", f.code, f.label));
        }
        report.push('\n');
    }
    if a.has("--dry-run") {
        if format != Format::Json && fixed != src {
            report.push_str("--- would write:\n");
            report.push_str(&fixed);
            if !fixed.ends_with('\n') {
                report.push('\n');
            }
        }
    } else if fixed != src {
        if let Err(e) = fix::write_atomic(Path::new(input), &fixed) {
            eprintln!("error: writing `{input}`: {e}");
            return 1;
        }
    }
    report.push_str(&render_all(&remaining, input, &fixed, format, usize::MAX));
    out(&report);
    i32::from(remaining.iter().any(Diagnostic::is_error))
}

fn cmd_explain(args: &[String]) -> i32 {
    let [code] = args else {
        return usage_error("give one error code, like `om explain E0200`");
    };
    match codes::find(&code.to_ascii_uppercase()) {
        Some(c) => {
            out(&format!("{}: {}\n\n{}\n", c.code, c.title, c.explain.trim_end()));
            0
        }
        None => {
            eprintln!("error: unknown code `{code}`; `om guide --codes` lists them all");
            1
        }
    }
}

fn cmd_doc(args: &[String]) -> i32 {
    let a = match Args::parse(args, &[], &["--file"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    if a.positional.len() > 1 {
        return usage_error(&format!("unexpected argument `{}`", a.positional[1]));
    }
    let user = match a.value("--file") {
        Some(f) => match load(f) {
            Ok(src) => Some((om_parser::parse_all(&src).0, f.to_string())),
            Err(c) => return c,
        },
        None => None,
    };
    let user_ref = user.as_ref().map(|(p, f)| (p, f.as_str()));
    match a.positional.first() {
        None => {
            out(&doc::overview(user_ref));
            0
        }
        Some(q) => match doc::lookup(q, user_ref) {
            Ok(s) => {
                out(&s);
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        },
    }
}

fn cmd_guide(args: &[String]) -> i32 {
    match args {
        [] => out(guide::GUIDE),
        [f] if f == "--codes" => out(&guide::codes()),
        _ => return usage_error("`om guide` takes only `--codes`"),
    }
    0
}

fn cmd_test(args: &[String]) -> i32 {
    let a = match Args::parse(args, &["--json", "--no-cache"], &["--timeout"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let timeout = match a.value("--timeout").map(str::parse::<f64>) {
        None => Duration::from_secs(10),
        Some(Ok(s)) if s.is_finite() && s > 0.0 && s <= 86_400.0 => Duration::from_secs_f64(s),
        Some(_) => return usage_error("`--timeout` needs a number of seconds"),
    };
    let src = match load(input) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let opts = testing::Options { use_cache: !a.has("--no-cache"), timeout };
    match testing::run(&src, &opts) {
        Ok(results) => {
            out(&testing::report(&results, a.has("--json")));
            let failed =
                results.iter().any(|r| matches!(r.outcome, testing::Outcome::Fail(..) | testing::Outcome::Timeout));
            i32::from(failed)
        }
        Err(diags) => {
            let format = if a.has("--json") { Format::Json } else { format_of(&a) };
            out(&render_all(&diags, input, &src, format, usize::MAX));
            1
        }
    }
}

fn cmd_fmt(args: &[String]) -> i32 {
    let a = match Args::parse(args, &["--check"], &[]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let src = match load(input) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let formatted = fmt::format(&src);
    if formatted == src {
        return 0;
    }
    if a.has("--check") {
        out(&format!("{input} is not formatted; run `om fmt {input}`\n"));
        return 1;
    }
    if let Err(e) = fix::write_atomic(Path::new(input), &formatted) {
        eprintln!("error: writing `{input}`: {e}");
        return 1;
    }
    0
}

fn cmd_audit(args: &[String]) -> i32 {
    let a = match Args::parse(args, &["--json"], &[]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let src = match load(input) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let (program, errors) = om_parser::parse_all(&src);
    if !errors.is_empty() {
        let diags = omc::check(&src);
        out(&render_all(&diags, input, &src, format_of(&a), usize::MAX));
        return 1;
    }
    out(&omc::audit::report(&program, input, a.has("--json")));
    0
}

fn cmd_watch(args: &[String]) -> i32 {
    let a = match Args::parse(args, &["--json", "--brief", "--human"], &[]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let input = match a.file() {
        Ok(f) => f,
        Err(e) => return usage_error(&e),
    };
    let format = format_of(&a);
    let mut last: Option<(SystemTime, u64)> = None;
    loop {
        let stamp = std::fs::metadata(input).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        if stamp.is_some() && stamp != last {
            last = stamp;
            let start = Instant::now();
            match omc::read_source(Path::new(input)) {
                Ok(src) => {
                    let diags = omc::check(&src);
                    let ms = start.elapsed().as_millis();
                    let mut s = render_all(&diags, input, &src, format, usize::MAX);
                    if format != Format::Json {
                        s.push_str(&format!("-- checked in {ms} ms; watching for changes\n"));
                    }
                    out(&s);
                }
                Err(e) => eprintln!("error: {e}"),
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn cmd_tokens(args: &[String]) -> i32 {
    if args.is_empty() {
        return usage_error("give one or more files");
    }
    let mut total = 0;
    for f in args {
        let src = match load(f) {
            Ok(s) => s,
            Err(c) => return c,
        };
        let n = tokens::count(&src);
        total += n;
        out(&format!("{n}\t{f}\n"));
    }
    if args.len() > 1 {
        out(&format!("{total}\ttotal\n"));
    }
    0
}
