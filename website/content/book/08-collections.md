# 8. Collections

Om's standard library has three heap types today: `Vec[T]` for lists,
`String` for text (with `str` as a view of text), and `Box[T]` for a single
value. All three are written in Om itself, in the compiler's prelude.
`HashMap` and friends are next on the roadmap.

## Vec

A `Vec[T]` is a list that can grow. Its elements are stored next to each
other on the heap.

```om
fn main() {
    let mut v: Vec[i64] = Vec::new()
    v.push(10)
    v.push(20)
    v.push(30)
    print(v)
    print(v.len())
    print(v[1])

    v[1] = 25
    v.insert(0, 5)
    print(v)
    print(v.pop())
    print(v.remove(0))
    print(v)
}
```

```console
$ om run main.om
[10, 20, 30]
3
20
[5, 10, 25, 30]
Some(30)
5
[10, 25]
```

List literals make a `Vec` directly: `[1, 2, 3]`, or `[0; 5]` for five zeros.

### Reading safely

`v[i]` panics if `i` is past the end. `get`, `first` and `last` return an
`Option` instead:

```om
fn main() {
    let v = [1, 2, 3]
    print(v.get(1))
    print(v.get(10))
    print(v.last())
}
```

```console
$ om run main.om
Some(2)
None
Some(3)
```

### Looping

```om
fn main() {
    let mut names = ["cara", "ali", "bo"]
    names.sort()
    for name in &names {
        print(name)
    }
    print(names.contains(&"bo"))
}
```

```console
$ om run main.om
ali
bo
cara
true
```

`for x in &list` borrows each element; the list is still usable after the loop.
`for x in list` moves each element out, and the list is used up.

### Other methods

`with_capacity`, `capacity`, `is_empty`, `clear`, `truncate`, `swap`,
`swap_remove`, `reverse`, `append`, `extend_from_slice`, `dedup` and a stable
`sort`. The [language reference](https://github.com/agnsha/om/blob/main/docs/language.md#ownership-vec-string-and-box)
lists them all.

## String and str

Om has two text types, the same split as Rust:

| | `String` | `str` |
|---|---|---|
| What it is | Text that owns its bytes on the heap | A view of text: a pointer and a length |
| Can it grow? | Yes | No |
| String literals | | `"hello"` is a `str` |
| Like Rust's | `String` | `&str` |

```om
fn main() {
    let mut s = String::from("Hello")
    s.push_str(", world")
    s.push('!')
    s += " :)"
    print(s)
    print(s.len())

    let greeting = "hi".to_string()
    let shout = greeting.to_uppercase()
    print(shout)
}
```

```console
$ om run main.om
Hello, world! :)
16
HI
```

### Passing text to functions

A function that only reads text should take `str`. Callers pass a literal
directly, or a `String` with `&`:

```om
fn count_vowels(text: str) -> i64 {
    let mut n = 0
    for c in text.chars() {
        if "aeiou".contains(c.to_string().as_str()) {
            n += 1
        }
    }
    n
}

fn main() {
    let owned = String::from("programming")
    print(count_vowels("hello"))
    print(count_vowels(&owned))
}
```

```console
$ om run main.om
2
3
```

### Searching and splitting

```om
fn main() {
    let line = "  name=Om; version=0.1  "
    let trimmed = line.trim()
    print(trimmed.starts_with("name"))
    print(trimmed.find(";"))

    for part in trimmed.split("; ") {
        if let Some((key, value)) = part.split_once("=") {
            print("{key} -> {value}")
        }
    }

    print("a-b-c".replace("-", "+"))
    print("ab".repeat(3))
    print("Om".slice(0, 1))
}
```

```console
$ om run main.om
true
Some(7)
name -> Om
version -> 0.1
a+b+c
ababab
O
```

`split`, `split_whitespace`, `lines` and `chars` return a `Vec` for now; they
will return iterators once Om has them.

### Building strings

String interpolation works in any string literal and accepts any expression:

```om
fn main() {
    let items = 3
    let price = 4.5
    let line = "{items} items at {price} = {items as f64 * price}"
    print(line)
    print(42.to_string() + "!")
}
```

```console
$ om run main.om
3 items at 4.5 = 13.5
42!
```

### Numbers from text

`parse_int()` and `parse_float()` return a `Result`:

```om
fn main() {
    print("123".parse_int())
    print("1.5".parse_float())
    print("abc".parse_int().is_err())
}
```

```console
$ om run main.om
Ok(123)
Ok(1.5)
true
```

### A str can't outlive its String

A `str` taken from a `String` points into it, so the `String` must stay alive.
Om checks this:

```om
fn first_three() -> str {
    let s = String::from("hello")
    s.slice(0, 3)
}

fn main() {
    print(first_three())
}
```

```text
error: cannot return a `str` that points into `s`, which is dropped when the function returns; return a String (`.to_string()`) instead
```

Return a `String` (`s.slice(0, 3).to_string()`) when the text needs to outlive
where it came from.

**Note:** `to_uppercase` and `to_lowercase` only change ASCII letters for now.

## Box

`Box[T]` puts one value on the heap. You need it when a type contains itself,
because otherwise its size would be infinite:

```om
enum Tree {
    Leaf(i64),
    Node(Box[Tree], Box[Tree]),
}

fn sum(t: &Tree) -> i64 {
    match t {
        Leaf(n) => n,
        Node(left, right) => sum(&*left) + sum(&*right),
    }
}

fn main() {
    let t = Tree::Node(
        Box::new(Tree::Leaf(1)),
        Box::new(Tree::Node(Box::new(Tree::Leaf(2)), Box::new(Tree::Leaf(3)))),
    )
    print(sum(&t))
}
```

```console
$ om run main.om
6
```

`*b` reads or replaces the boxed value. Fields and methods work through a box
directly, so `b.x` and `b.area()` need no `*`.

## Try it

- Write `fn longest_len(words: &Vec[str]) -> i64` that returns the length of
  the longest word.
- Write a word counter: split a sentence with `split_whitespace`, sort the
  words, and print each with how many times it appears.
- Add a `fn depth(t: &Tree) -> i64` to the tree example.

Next: [Generics and traits](09-generics-traits.md).
