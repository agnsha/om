fn main() {
    let n: i64 = std::env::args().nth(1).expect("usage: collatz N").parse().expect("N");
    let mut best = 0;
    let mut best_len = 0;
    for start in 1..n {
        let mut x = start;
        let mut len = 1;
        while x != 1 {
            x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
            len += 1;
        }
        if len > best_len {
            best_len = len;
            best = start;
        }
    }
    println!("{} {}", best, best_len);
}
