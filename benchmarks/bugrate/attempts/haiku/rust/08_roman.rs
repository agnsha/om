fn to_roman(mut n: u32) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
        (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
        (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut out = String::new();
    for &(value, symbol) in TABLE.iter() {
        while n >= value {
            out.push_str(symbol);
            n -= value;
        }
    }
    out
}

fn main() {
    for n in [1994, 2024, 4, 9, 3999, 58] {
        println!("{} = {}", n, to_roman(n));
    }
}
