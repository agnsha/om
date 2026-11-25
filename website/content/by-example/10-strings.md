# String and str

`String` owns text and `str` is a view of it. Interpolation accepts any expression.

```om
// String owns text; str is a view of text (like Rust's &str).
fn shout(s: str) -> String {
    let mut out = s.to_uppercase()
    out.push('!')
    out
}

fn main() {
    let mut s = String::from("hello")
    s.push_str(", world")
    s += "."
    print(s)
    print(s.len())
    print(s.contains("world"))
    print(s.replace("world", "Om"))
    print(shout(&s))

    let line = "  name = Om  "
    if let Some((k, v)) = line.trim().split_once(" = ") {
        print("key {k}, value {v}")
    }

    for word in "one two three".split_whitespace() {
        print(word)
    }

    let n = 7
    let msg = "n squared is {n * n}"
    print(msg)
    print(n.to_string() + " items")
}
```

Run it:

```console
$ om run 10-strings.om
hello, world.
13
true
hello, Om.
HELLO, WORLD.!
key name, value Om
one
two
three
n squared is 49
7 items
```

The source is in [`10-strings.om`](10-strings.om). Back to [Om by Example](README.md).
