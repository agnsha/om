### 01_fizzbuzz
```om
fn main() {
    for i in 1..=15 {
        if i % 15 == 0 {
            print("FizzBuzz")
        } else if i % 3 == 0 {
            print("Fizz")
        } else if i % 5 == 0 {
            print("Buzz")
        } else {
            print(i)
        }
    }
}
```

### 02_word_count
```om
fn main() {
    let text = "the quick brown fox jumps over the lazy dog the fox"
    let words = text.split(" ")
    let mut distinct: Vec[str] = Vec::new()
    let mut counts: Vec[i64] = Vec::new()
    for w in &words {
        let mut found = false
        let mut i = 0
        while i < distinct.len() {
            if distinct[i] == w {
                counts[i] += 1
                found = true
                break
            }
            i += 1
        }
        if !found {
            distinct.push(w)
            counts.push(1)
        }
    }
    let mut i = 0
    while i < distinct.len() {
        print("{distinct[i]}: {counts[i]}")
        i += 1
    }
}
```

### 03_bank
```om
struct Account { owner: String, balance: i64 }

impl Account {
    fn new(owner: str, balance: i64) -> Self {
        Account { owner: owner.to_string(), balance }
    }

    fn deposit(&mut self, amount: i64) -> i64 {
        self.balance += amount
        self.balance
    }

    fn withdraw(&mut self, amount: i64) -> Result[i64, str] {
        if amount > self.balance {
            return Err("insufficient funds")
        }
        self.balance -= amount
        Ok(self.balance)
    }
}

fn report(r: Result[i64, str]) {
    match r {
        Ok(b) => print("ok {b}"),
        Err(e) => print("error: {e}"),
    }
}

fn main() {
    let mut acct = Account::new("ada", 100)
    report(Ok(acct.deposit(50)))
    report(acct.withdraw(30))
    report(acct.withdraw(500))
    report(Ok(acct.deposit(5)))
    print("{acct.owner} has {acct.balance}")
}
```

### 04_shapes
```om
enum Shape { Square(i64), Rect(i64, i64), Triangle(i64, i64) }

impl Shape {
    fn area(&self) -> i64 {
        match self {
            Square(s) => s * s,
            Rect(w, h) => w * h,
            Triangle(b, h) => b * h / 2,
        }
    }
}

fn main() {
    let shapes = [Shape::Square(3), Shape::Rect(2, 5), Shape::Triangle(4, 3), Shape::Square(1)]
    let mut total = 0
    let mut largest = 0
    for s in &shapes {
        let a = s.area()
        print(a)
        total += a
        largest = largest.max(a)
    }
    print("total {total}")
    print("largest {largest}")
}
```

### 05_stack
```om
struct Stack[T] { items: Vec[T] }

impl[T: Clone] Stack[T] {
    fn new() -> Self { Stack { items: Vec::new() } }

    fn push(&mut self, item: T) { self.items.push(item) }

    fn pop(&mut self) -> Option[T] { self.items.pop() }

    fn peek(&self) -> Option[T] {
        if self.items.len() == 0 {
            return None
        }
        Some(self.items[self.items.len() - 1].clone())
    }

    fn len(&self) -> i64 { self.items.len() as i64 }
}

fn main() {
    let mut s: Stack[i64] = Stack::new()
    s.push(1)
    s.push(2)
    s.push(3)
    print("len {s.len()}")
    match s.peek() {
        Some(v) => print("peek {v}"),
        None => print("peek none"),
    }
    loop {
        match s.pop() {
            Some(v) => print("pop {v}"),
            None => {
                print("empty")
                break
            }
        }
    }
    let mut t: Stack[String] = Stack::new()
    t.push("a".to_string())
    t.push("b".to_string())
    match t.peek() {
        Some(v) => print("top {v}"),
        None => print("top none"),
    }
}
```

### 06_parse_ages
```om
fn main() {
    let lines = ["alice,30", "bob,x", "carol,25", "dave", "erin,-4"]
    let mut sum = 0
    for line in &lines {
        let parts = line.split(",")
        if parts.len() != 2 {
            print("bad line: {line}")
            continue
        }
        let name = parts[0]
        match parts[1].trim().parse_int() {
            Ok(age) => {
                if age < 0 {
                    print("negative age for {name}")
                } else {
                    print("{name} is {age}")
                    sum += age
                }
            }
            Err(_) => print("bad age for {name}"),
        }
    }
    print("sum {sum}")
}
```

### 07_binary_search
```om
fn search(v: &Vec[i64], target: i64) -> Option[i64] {
    let mut lo: i64 = 0
    let mut hi: i64 = v.len() as i64 - 1
    while lo <= hi {
        let mid = lo + (hi - lo) / 2
        let x = v[mid as usize]
        if x == target {
            return Some(mid)
        } else if x < target {
            lo = mid + 1
        } else {
            hi = mid - 1
        }
    }
    None
}

fn main() {
    let v = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19]
    let queries = [7, 1, 19, 4, 20]
    for q in &queries {
        match search(&v, q) {
            Some(i) => print("{q} at {i}"),
            None => print("{q} missing"),
        }
    }
}
```

### 08_roman
```om
fn to_roman(n: i64) -> String {
    let vals = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1]
    let syms = ["M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"]
    let mut rest = n
    let mut out = String::new()
    let mut i = 0
    while i < vals.len() {
        while rest >= vals[i] {
            out.push_str(syms[i])
            rest -= vals[i]
        }
        i += 1
    }
    out
}

fn main() {
    let nums = [1994, 2024, 4, 9, 3999, 58]
    for n in &nums {
        print("{n} = {to_roman(n)}")
    }
}
```

### 09_brackets
```om
fn is_valid(s: str) -> bool {
    let mut stack: Vec[char] = Vec::new()
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => stack.push(c),
            ')' | ']' | '}' => {
                let want = if c == ')' { '(' } else if c == ']' { '[' } else { '{' }
                match stack.pop() {
                    Some(top) => {
                        if top != want {
                            return false
                        }
                    }
                    None => return false,
                }
            }
            _ => {}
        }
    }
    stack.len() == 0
}

fn main() {
    let cases = ["([]{})", "([)]", "((", "{[()()]}", ")("]
    for s in &cases {
        if is_valid(s) {
            print("{s} valid")
        } else {
            print("{s} invalid")
        }
    }
}
```

### 10_primes
```om
fn is_prime(n: i64) -> bool {
    if n < 2 {
        return false
    }
    let mut d = 2
    while d * d <= n {
        if n % d == 0 {
            return false
        }
        d += 1
    }
    true
}

fn main() {
    let mut line = String::new()
    let mut found = 0
    let mut n = 2
    while found < 10 {
        if is_prime(n) {
            if found > 0 {
                line.push_str(" ")
            }
            line.push_str("{n}")
            found += 1
        }
        n += 1
    }
    print(line)

    let mut count = 0
    for k in 2..10000 {
        if is_prime(k) {
            count += 1
        }
    }
    print("count {count}")
}
```

### 11_expr_tree
```om
enum Expr {
    Num(i64),
    Add(Box[Expr], Box[Expr]),
    Sub(Box[Expr], Box[Expr]),
    Mul(Box[Expr], Box[Expr]),
}

impl Expr {
    fn eval(&self) -> i64 {
        match self {
            Num(n) => n,
            Add(a, b) => a.eval() + b.eval(),
            Sub(a, b) => a.eval() - b.eval(),
            Mul(a, b) => a.eval() * b.eval(),
        }
    }

    fn depth(&self) -> i64 {
        match self {
            Num(_) => 1,
            Add(a, b) => 1 + a.depth().max(b.depth()),
            Sub(a, b) => 1 + a.depth().max(b.depth()),
            Mul(a, b) => 1 + a.depth().max(b.depth()),
        }
    }
}

fn num(n: i64) -> Box[Expr] { Box::new(Expr::Num(n)) }

fn main() {
    let first = Expr::Mul(
        Box::new(Expr::Add(num(2), num(3))),
        Box::new(Expr::Sub(num(7), num(4))),
    )
    let second = Expr::Sub(
        num(10),
        Box::new(Expr::Mul(num(2), num(3))),
    )
    print(first.eval())
    print(second.eval())
    print("depth {first.depth()}")
}
```

### 12_caesar
```om
fn shift(text: str, k: i64) -> String {
    let mut out = String::new()
    for c in text.chars() {
        let code = c as u8 as i64
        if code >= 97 && code <= 122 {
            let r = (code - 97 + k) % 26
            out.push((97 + r) as u8 as char)
        } else if code >= 65 && code <= 90 {
            let r = (code - 65 + k) % 26
            out.push((65 + r) as u8 as char)
        } else {
            out.push(c)
        }
    }
    out
}

fn main() {
    let a = shift("Hello, World!", 3)
    print(a)
    print(shift(&a, 23))
    print(shift("xyz ABC", 4))
}
```
