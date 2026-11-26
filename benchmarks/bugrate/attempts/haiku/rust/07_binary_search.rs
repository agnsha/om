fn binary_search(sorted: &[i32], target: i32) -> Option<usize> {
    let mut low = 0;
    let mut high = sorted.len();
    while low < high {
        let mid = low + (high - low) / 2;
        if sorted[mid] == target {
            return Some(mid);
        } else if sorted[mid] < target {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    None
}

fn main() {
    let values: Vec<i32> = (0..10).map(|i| 2 * i + 1).collect();
    for query in [7, 1, 19, 4, 20] {
        match binary_search(&values, query) {
            Some(index) => println!("{} at {}", query, index),
            None => println!("{} missing", query),
        }
    }
}
