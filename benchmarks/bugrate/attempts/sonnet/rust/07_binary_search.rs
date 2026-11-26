fn binary_search(items: &[i32], target: i32) -> Option<usize> {
    let mut lo = 0;
    let mut hi = items.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if items[mid] == target {
            return Some(mid);
        } else if items[mid] < target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    None
}

fn main() {
    let items = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19];
    for q in [7, 1, 19, 4, 20] {
        match binary_search(&items, q) {
            Some(i) => println!("{} at {}", q, i),
            None => println!("{} missing", q),
        }
    }
}
