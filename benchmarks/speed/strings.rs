use std::fmt::Write;

fn main() {
    let n: i64 = std::env::args().nth(1).expect("usage: strings N").parse().expect("N");
    let mut s = String::new();
    for i in 0..n {
        write!(s, "{},", i).unwrap();
    }
    let mut sevens = 0;
    for b in s.bytes() {
        if b == b'7' {
            sevens += 1;
        }
    }
    println!("{} {}", s.len(), sevens);
}
