fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = 2;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 1;
    }
    true
}

fn main() {
    let first: Vec<String> = (2..)
        .filter(|&n| is_prime(n))
        .take(10)
        .map(|n| n.to_string())
        .collect();
    println!("{}", first.join(" "));

    let count = (2..10000).filter(|&n| is_prime(n)).count();
    println!("count {}", count);
}
