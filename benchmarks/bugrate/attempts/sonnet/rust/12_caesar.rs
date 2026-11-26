fn shift(text: &str, k: i32) -> String {
    let k = k.rem_euclid(26) as u8;
    text.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                (((c as u8 - b'a' + k) % 26) + b'a') as char
            } else if c.is_ascii_uppercase() {
                (((c as u8 - b'A' + k) % 26) + b'A') as char
            } else {
                c
            }
        })
        .collect()
}

fn main() {
    let encoded = shift("Hello, World!", 3);
    println!("{}", encoded);
    println!("{}", shift(&encoded, 23));
    println!("{}", shift("xyz ABC", 4));
}
