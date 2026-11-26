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
```

### 03_bank
```rust
struct Account {
    owner: String,
    balance: i64,
}

impl Account {
    fn new(owner: &str, balance: i64) -> Self {
        Account {
            owner: owner.to_string(),
            balance,
        }
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

fn report(result: Result<(), String>, account: &Account) {
    match result {
        Ok(()) => println!("ok {}", account.balance),
        Err(msg) => println!("error: {}", msg),
    }
}

fn main() {
    let mut account = Account::new("ada", 100);

    account.deposit(50);
    report(Ok(()), &account);

    let r = account.withdraw(30);
    report(r, &account);

    let r = account.withdraw(500);
    report(r, &account);

    account.deposit(5);
    report(Ok(()), &account);

    println!("{} has {}", account.owner, account.balance);
}
```

### 04_shapes
```rust
enum Shape {
    Square(i64),
    Rect(i64, i64),
    Triangle(i64, i64),
}

impl Shape {
    fn area(&self) -> i64 {
        match self {
            Shape::Square(s) => s * s,
            Shape::Rect(w, h) => w * h,
            Shape::Triangle(b, h) => b * h / 2,
        }
    }
}

fn main() {
    let shapes = [
        Shape::Square(3),
        Shape::Rect(2, 5),
        Shape::Triangle(4, 3),
        Shape::Square(1),
    ];
    let areas: Vec<i64> = shapes.iter().map(|s| s.area()).collect();
    for a in &areas {
        println!("{}", a);
    }
    let total: i64 = areas.iter().sum();
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
    let mut nums: Stack<i32> = Stack::new();
    nums.push(1);
    nums.push(2);
    nums.push(3);
    println!("len {}", nums.len());
    if let Some(top) = nums.peek() {
        println!("peek {}", top);
    }
    while let Some(v) = nums.pop() {
        println!("pop {}", v);
    }
    println!("empty");

    let mut words: Stack<String> = Stack::new();
    words.push("a".to_string());
    words.push("b".to_string());
    if let Some(top) = words.peek() {
        println!("top {}", top);
    }
}
```

### 06_parse_ages
```rust
fn main() {
    let lines = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"];
    let mut sum: i64 = 0;
    for line in lines {
        let Some((name, age_str)) = line.split_once(',') else {
            println!("bad line: {}", line);
            continue;
        };
        match age_str.parse::<i64>() {
            Err(_) => println!("bad age for {}", name),
            Ok(age) if age < 0 => println!("negative age for {}", name),
            Ok(age) => {
                println!("{} is {}", name, age);
                sum += age;
            }
        }
    }
    println!("sum {}", sum);
}
```

### 07_binary_search
```rust
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
```

### 08_roman
```rust
fn to_roman(mut n: u32) -> String {
    const TABLE: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
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
    let mut stack: Vec<char> = Vec::new();
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
        if is_balanced(s) {
            println!("{} valid", s);
        } else {
            println!("{} invalid", s);
        }
    }
}
```

### 10_primes
```rust
fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = 2;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 1;
    }
    true
}

fn main() {
    let first: Vec<String> = (2..)
        .filter(|&n| is_prime(n))
        .take(10)
        .map(|n| n.to_string())
        .collect();
    println!("{}", first.join(" "));

    let count = (2..10000).filter(|&n| is_prime(n)).count();
    println!("count {}", count);
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

use Expr::*;

fn lit(n: i64) -> Box<Expr> {
    Box::new(Lit(n))
}

fn eval(e: &Expr) -> i64 {
    match e {
        Lit(n) => *n,
        Add(a, b) => eval(a) + eval(b),
        Sub(a, b) => eval(a) - eval(b),
        Mul(a, b) => eval(a) * eval(b),
    }
}

fn depth(e: &Expr) -> usize {
    match e {
        Lit(_) => 1,
        Add(a, b) | Sub(a, b) | Mul(a, b) => 1 + depth(a).max(depth(b)),
    }
}

fn main() {
    // (2 + 3) * (7 - 4)
    let first = Mul(
        Box::new(Add(lit(2), lit(3))),
        Box::new(Sub(lit(7), lit(4))),
    );
    // 10 - 2 * 3
    let second = Sub(lit(10), Box::new(Mul(lit(2), lit(3))));

    println!("{}", eval(&first));
    println!("{}", eval(&second));
    println!("depth {}", depth(&first));
}
```

### 12_caesar
```rust
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
```
