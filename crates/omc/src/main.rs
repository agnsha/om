//! `om`: the command line tool for the Om language.

use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "\
om: the Om language compiler

Usage:
  om build <file.om> [-o <output>]   compile to a native executable
  om run <file.om>                   compile and run
  om check <file.om>                 parse and type-check only
  om --version                       print the version
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("build") => cmd_build(&args[1..]),
        Some("run") => cmd_run(&args[1..]),
        Some("check") => cmd_check(&args[1..]),
        Some("--version" | "-V") => {
            println!("om {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("--help" | "-h" | "help") => {
            print!("{USAGE}");
            0
        }
        _ => {
            eprint!("{USAGE}");
            2
        }
    };
    exit(code);
}

fn read_source(path: &str) -> Result<String, i32> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("error: cannot read `{path}`: {e}");
        1
    })
}

fn default_output(input: &str) -> PathBuf {
    let stem = Path::new(input).file_stem().and_then(|s| s.to_str()).unwrap_or("a");
    PathBuf::from(if cfg!(windows) { format!("{stem}.exe") } else { stem.to_string() })
}

fn build_to(input: &str, out: &Path) -> Result<(), i32> {
    let src = read_source(input)?;
    omc::build(&src, out).map_err(|d| {
        eprintln!("{}", d.render(input, &src));
        1
    })
}

fn cmd_build(args: &[String]) -> i32 {
    let Some(input) = args.first() else {
        eprint!("{USAGE}");
        return 2;
    };
    let out = match args.get(1).map(String::as_str) {
        Some("-o") => match args.get(2) {
            Some(o) => PathBuf::from(o),
            None => {
                eprintln!("error: `-o` needs a file name");
                return 2;
            }
        },
        _ => default_output(input),
    };
    match build_to(input, &out) {
        Ok(()) => 0,
        Err(c) => c,
    }
}

fn cmd_run(args: &[String]) -> i32 {
    let Some(input) = args.first() else {
        eprint!("{USAGE}");
        return 2;
    };
    let out = std::env::temp_dir().join(format!("om-run-{}", std::process::id()));
    if let Err(c) = build_to(input, &out) {
        return c;
    }
    let status = Command::new(&out).args(&args[1..]).status();
    let _ = std::fs::remove_file(&out);
    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("error: could not start the program: {e}");
            1
        }
    }
}

fn cmd_check(args: &[String]) -> i32 {
    let Some(input) = args.first() else {
        eprint!("{USAGE}");
        return 2;
    };
    let src = match read_source(input) {
        Ok(s) => s,
        Err(c) => return c,
    };
    match omc::compile(&src) {
        Ok(_) => {
            println!("ok: {input}");
            0
        }
        Err(d) => {
            eprintln!("{}", d.render(input, &src));
            1
        }
    }
}
