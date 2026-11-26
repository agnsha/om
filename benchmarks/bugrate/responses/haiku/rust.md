### 01_fizzbuzz
```rust
fn main() {
    for n in 1..=15 {
        if n % 15 == 0 {
            println!("FizzBuzz");
        } else if n % 3 == 0 {
            println!("Fizz");
        } else if n % 5 == 0 {
            println!("Buzz");
        } else {
            println!("{}", n);
        }
    }
}
```

### 02_word_count
```rust
use std::collections::HashMap;

fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox";
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
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
```

### 03_bank
```rust
struct Account {
    owner: String,
    balance: i64,
}

impl Account {
    fn new(owner: &str, balance: i64) -> Self {
        Account { owner: owner.to_string(), balance }
    }

    fn deposit(&mut self, amount: i64) {
        self.balance += amount;
    }

    fn withdraw(&mut self, amount: i64) -> Result<(), String> {
        if amount > self.balance {
            return Err("insufficient funds".to_string());
        }
        self.balance -= amount;
        Ok(())
    }
}

fn main() {
    let mut account = Account::new("ada", 100);

    account.deposit(50);
    println!("ok {}", account.balance);

    match account.withdraw(30) {
        Ok(()) => println!("ok {}", account.balance),
        Err(e) => println!("error: {}", e),
    }
    match account.withdraw(500) {
        Ok(()) => println!("ok {}", account.balance),
        Err(e) => println!("error: {}", e),
    }
    account.deposit(5);
    println!("ok {}", account.balance);

    println!("{} has {}", account.owner, account.balance);
}
```

### 04_shapes
```rust
enum Shape {
    Square(i32),
    Rect(i32, i32),
    Triangle(i32, i32),
}

impl Shape {
    fn area(&self) -> i32 {
        match *self {
            Shape::Square(side) => side * side,
            Shape::Rect(width, height) => width * height,
            Shape::Triangle(base, height) => base * height / 2,
        }
    }
}

fn main() {
    let shapes = vec![
        Shape::Square(3),
        Shape::Rect(2, 5),
        Shape::Triangle(4, 3),
        Shape::Square(1),
    ];
    let areas: Vec<i32> = shapes.iter().map(|s| s.area()).collect();
    for area in &areas {
        println!("{}", area);
    }
    let total: i32 = areas.iter().sum();
    let largest = areas.iter().max().unwrap();
    println!("total {}", total);
    println!("largest {}", largest);
}
```

### 05_stack
```rust
struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    fn new() -> Self {
        Stack { items: Vec::new() }
    }

    fn push(&mut self, item: T) {
        self.items.push(item);
    }

    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    fn peek(&self) -> Option<&T> {
        self.items.last()
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}

fn main() {
    let mut numbers = Stack::new();
    numbers.push(1);
    numbers.push(2);
    numbers.push(3);

    println!("len {}", numbers.len());
    println!("peek {}", numbers.peek().unwrap());
    while let Some(value) = numbers.pop() {
        println!("pop {}", value);
    }
    println!("empty");

    let mut words = Stack::new();
    words.push("a");
    words.push("b");
    println!("top {}", words.peek().unwrap());
}
```

### 06_parse_ages
```rust
fn main() {
    let lines = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"];
    let mut sum: i64 = 0;

    for line in lines {
        let Some((name, age_text)) = line.split_once(',') else {
            println!("bad line: {}", line);
            continue;
        };
        match age_text.parse::<i64>() {
            Ok(age) if age < 0 => println!("negative age for {}", name),
            Ok(age) => {
                println!("{} is {}", name, age);
                sum += age;
            }
            Err(_) => println!("bad age for {}", name),
        }
    }

    println!("sum {}", sum);
}
```

### 07_binary_search
```rust
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
```

### 08_roman
```rust
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
```

### 09_brackets
```rust
fn is_balanced(s: &str) -> bool {
    let mut stack = Vec::new();
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => stack.push(c),
            ')' | ']' | '}' => {
                let expected = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if stack.pop() != Some(expected) {
                    return false;
                }
            }
            _ => {}
        }
    }
    stack.is_empty()
}

fn main() {
    for s in ["([]{})", "([)]", "((", "{[()()]}", ")("] {
        let verdict = if is_balanced(s) { "valid" } else { "invalid" };
        println!("{} {}", s, verdict);
    }
}
```

### 10_primes
```rust
fn main() {
    let limit = 10_000;
    let mut is_prime = vec![true; limit];
    is_prime[0] = false;
    is_prime[1] = false;
    let mut i = 2;
    while i * i < limit {
        if is_prime[i] {
            let mut multiple = i * i;
            while multiple < limit {
                is_prime[multiple] = false;
                multiple += i;
            }
        }
        i += 1;
    }
    let primes: Vec<usize> = (2..limit).filter(|&n| is_prime[n]).collect();

    let first: Vec<String> = primes.iter().take(10).map(|p| p.to_string()).collect();
    println!("{}", first.join(" "));
    println!("count {}", primes.len());
}
```

### 11_expr_tree
```rust
enum Expr {
    Lit(i64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval(&self) -> i64 {
        match self {
            Expr::Lit(n) => *n,
            Expr::Add(a, b) => a.eval() + b.eval(),
            Expr::Sub(a, b) => a.eval() - b.eval(),
            Expr::Mul(a, b) => a.eval() * b.eval(),
        }
    }

    fn depth(&self) -> usize {
        match self {
            Expr::Lit(_) => 1,
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) => 1 + a.depth().max(b.depth()),
        }
    }
}

fn main() {
    // (2 + 3) * (7 - 4)
    let first = Expr::Mul(
        Box::new(Expr::Add(Box::new(Expr::Lit(2)), Box::new(Expr::Lit(3)))),
        Box::new(Expr::Sub(Box::new(Expr::Lit(7)), Box::new(Expr::Lit(4)))),
    );
    // 10 - 2 * 3
    let second = Expr::Sub(
        Box::new(Expr::Lit(10)),
        Box::new(Expr::Mul(Box::new(Expr::Lit(2)), Box::new(Expr::Lit(3)))),
    );

    println!("{}", first.eval());
    println!("{}", second.eval());
    println!("depth {}", first.depth());
}
```

### 12_caesar
```rust
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
```
