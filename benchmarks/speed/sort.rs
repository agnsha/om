fn main() {
    let n: i64 = std::env::args().nth(1).expect("usage: sort N").parse().expect("N");
    let mut v: Vec<i64> = Vec::with_capacity(n as usize);
    let mut seed: i64 = 42;
    for _ in 0..n {
        seed = (seed * 1103515245 + 12345) % 2147483648;
        v.push(seed);
    }
    v.sort();
    let mut check = 0;
    for i in 0..v.len() {
        if i % 1000 == 0 {
            check = (check + v[i]) % 1000000007;
        }
    }
    println!("{}", check);
}
