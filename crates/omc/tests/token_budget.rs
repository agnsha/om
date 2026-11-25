//! Token budget: the same small programs in Om, Rust and Python
//! (`bench/tokens`). Om must stay shorter than Rust and close to Python, so a
//! model spends fewer tokens reading and writing it. Counts use the
//! approximate tokenizer in `omc::tokens`, which is good for comparisons.

use std::path::Path;
use std::process::{Command, Stdio};

/// Om may use at most this share of Rust's tokens, over all tasks.
const MAX_VS_RUST: f64 = 0.90;
/// And at most this multiple of Python's.
const MAX_VS_PYTHON: f64 = 1.10;

#[test]
fn om_programs_stay_within_the_token_budget() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/tokens");
    let mut tasks: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension()? == "om").then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    tasks.sort();
    assert!(tasks.len() >= 5, "only {} tasks", tasks.len());
    let (mut om, mut rs, mut py) = (0, 0, 0);
    let mut table = String::new();
    for t in &tasks {
        let read = |ext: &str| {
            std::fs::read_to_string(dir.join(format!("{t}.{ext}"))).unwrap_or_else(|_| panic!("missing {t}.{ext}"))
        };
        let (src, rust, python) = (read("om"), read("rs"), read("py"));
        let n = [&src, &rust, &python].map(|s| omc::tokens::count(s));
        table.push_str(&format!("{t:12} om {:4}  rust {:4}  python {:4}\n", n[0], n[1], n[2]));
        assert!(n[0] <= n[1], "{t}: Om takes {} tokens, Rust {}", n[0], n[1]);
        (om, rs, py) = (om + n[0], rs + n[1], py + n[2]);

        // The Om version must be a working program that prints what the Rust one does.
        let tmp = omc::TempDir::new().unwrap();
        let exe = tmp.path().join(t);
        omc::build(&src, &exe).unwrap_or_else(|d| panic!("{}", d.render(t, &src)));
        let out = omc::spawn(Command::new(&exe).stdout(Stdio::piped())).and_then(|c| c.wait_with_output()).unwrap();
        assert!(out.status.success(), "{t} exited with {}", out.status);
        assert_eq!(String::from_utf8_lossy(&out.stdout), read("expected"), "output of {t}.om");
    }
    println!("{table}total        om {om:4}  rust {rs:4}  python {py:4}");
    assert!(om as f64 <= rs as f64 * MAX_VS_RUST, "Om {om} vs Rust {rs} tokens\n{table}");
    assert!(om as f64 <= py as f64 * MAX_VS_PYTHON, "Om {om} vs Python {py} tokens\n{table}");
}
