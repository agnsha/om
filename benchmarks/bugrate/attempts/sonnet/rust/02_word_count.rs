use std::collections::HashMap;

fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox";
    let mut order: Vec<&str> = Vec::new();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for word in text.split(' ') {
        let entry = counts.entry(word).or_insert(0);
        if *entry == 0 {
            order.push(word);
        }
        *entry += 1;
    }
    for word in order {
        println!("{}: {}", word, counts[word]);
    }
}
