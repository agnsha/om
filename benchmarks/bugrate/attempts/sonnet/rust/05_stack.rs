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
