fn main() {
    let limit = 10_000;
    let mut is_prime = vec![true; limit];
    is_prime[0] = false;
    is_prime[1] = false;
    let mut i = 2;
    while i * i < limit {
        if is_prime[i] {
            let mut multiple = i * i;
            while multiple < limit {
                is_prime[multiple] = false;
                multiple += i;
            }
        }
        i += 1;
    }
    let primes: Vec<usize> = (2..limit).filter(|&n| is_prime[n]).collect();

    let first: Vec<String> = primes.iter().take(10).map(|p| p.to_string()).collect();
    println!("{}", first.join(" "));
    println!("count {}", primes.len());
}
