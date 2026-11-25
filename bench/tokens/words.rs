fn long_words(text: &str, min: usize) -> Vec<String> {
    let mut out = Vec::new();
    for w in text.split_whitespace() {
        if w.len() >= min {
            out.push(w.to_uppercase());
        }
    }
    out
}

fn main() {
    let words = long_words("the quick brown fox jumps over the lazy dog", 5);
    println!("{:?}", words);
    println!("{} long words", words.len());
}
