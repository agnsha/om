fn shift(text: &str, k: i32) -> String {
    text.chars()
        .map(|c| {
            let base = if c.is_ascii_lowercase() {
                b'a'
            } else if c.is_ascii_uppercase() {
                b'A'
            } else {
                return c;
            };
            let offset = (c as u8 - base) as i32;
            let rotated = (offset + k).rem_euclid(26) as u8;
            (base + rotated) as char
        })
        .collect()
}

fn main() {
    let once = shift("Hello, World!", 3);
    println!("{}", once);
    println!("{}", shift(&once, 23));
    println!("{}", shift("xyz ABC", 4));
}
