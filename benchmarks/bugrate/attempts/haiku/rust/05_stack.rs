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
