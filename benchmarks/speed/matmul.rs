fn main() {
    let n: usize = std::env::args().nth(1).expect("usage: matmul N").parse().expect("N");
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n * n];
    let mut c = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (i + j) as f64 / n as f64;
            b[i * n + j] = (i as f64 - j as f64) / n as f64;
        }
    }
    for i in 0..n {
        for k in 0..n {
            let x = a[i * n + k];
            for j in 0..n {
                c[i * n + j] += x * b[k * n + j];
            }
        }
    }
    let mut sum = 0.0;
    for v in &c {
        sum += v;
    }
    println!("{:?}", sum);
}
