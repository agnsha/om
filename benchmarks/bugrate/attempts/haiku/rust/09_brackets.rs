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
