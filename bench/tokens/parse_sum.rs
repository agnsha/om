fn parse_all(items: &[&str]) -> Result<i64, String> {
    let mut sum = 0;
    for s in items {
        sum += s
            .trim()
            .parse::<i64>()
            .map_err(|e| format!("item `{}`: {}", s, e))?;
    }
    Ok(sum)
}

fn main() {
    println!("{:?}", parse_all(&["1", " 2", "39"]));
    println!("{:?}", parse_all(&["4", "x5"]));
}
