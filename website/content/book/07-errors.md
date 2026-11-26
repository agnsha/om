# 7. Handling errors

Om has no null and no exceptions. A value that might be missing has type
`Option[T]`, and an operation that might fail returns `Result[T, E]`. The types
make you deal with the bad case, and the compiler checks that you did.

## Option

```om
enum Option[T] { Some(T), None }
```

That's the real definition, written in Om in the standard prelude. `Some` and
`None` can be used without the `Option::` prefix.

```om
fn find(list: &Vec[i64], target: i64) -> Option[i64] {
    for i in 0..list.len() {
        if list[i] == target {
            return Some(i as i64)
        }
    }
    None
}

fn main() {
    let nums = [4, 8, 15, 16, 23, 42]
    print(find(&nums, 15))
    print(find(&nums, 99))

    match find(&nums, 23) {
        Some(i) => print("found at index {i}"),
        None => print("not there"),
    }

    print(find(&nums, 99).unwrap_or(-1))
}
```

```console
$ om run main.om
Some(2)
None
found at index 4
-1
```

Useful methods: `is_some`, `is_none`, `unwrap`, `expect`, `unwrap_or`, and
`ok_or` to turn an Option into a Result.

`while let` loops until a `None`. `Vec::pop` returns an Option, so it fits
well:

```om
fn main() {
    let mut stack = [1, 2, 3]
    while let Some(top) = stack.pop() {
        print(top)
    }
}
```

```console
$ om run main.om
3
2
1
```

## Result

```om
enum Result[T, E] { Ok(T), Err(E) }
```

```om
fn parse_age(text: str) -> Result[i64, str] {
    match text.parse_int() {
        Ok(n) => {
            if n < 0 || n > 150 {
                Err("age out of range")
            } else {
                Ok(n)
            }
        }
        Err(_) => Err("not a number"),
    }
}

fn main() {
    print(parse_age("34"))
    print(parse_age("-3"))
    print(parse_age("old"))
}
```

```console
$ om run main.om
Ok(34)
Err("age out of range")
Err("not a number")
```

Useful methods: `is_ok`, `is_err`, `unwrap`, `expect`, `unwrap_or`, `ok`
(to an Option) and `err`.

## The ? operator

Most functions that can fail call other functions that can fail. Matching on
each result gets long. `?` does it for you: on `Ok(v)` it gives you `v`; on
`Err(e)` it returns `Err(e)` from the current function right away.

```om
fn parse_pair(a: str, b: str) -> Result[(i64, i64), str] {
    let x = a.parse_int()?
    let y = b.parse_int()?
    Ok((x, y))
}

fn ratio(a: str, b: str) -> Result[i64, str] {
    let (x, y) = parse_pair(a, b)?
    if y == 0 {
        return Err("division by zero")
    }
    Ok(x / y)
}

fn main() {
    print(ratio("10", "2"))
    print(ratio("10", "zero"))
    print(ratio("10", "0"))
}
```

```console
$ om run main.om
Ok(5)
Err("invalid digit found in string")
Err("division by zero")
```

`?` works on `Option` too, in a function that returns an `Option`. The error
types must match: `?` on a `Result[T, str]` needs a function returning
`Result[_, str]`. (Automatic error conversion with `From` is planned.)

## Panics: when there's no sensible way on

Some failures are bugs, not situations to handle. For those, stop the program:

```om
fn average(list: &Vec[i64]) -> i64 {
    assert(list.len() > 0, "average of an empty list")
    let mut sum = 0
    for i in 0..list.len() {
        sum += list[i]
    }
    sum / list.len() as i64
}

fn main() {
    print(average(&[2, 4, 6]))
    let empty: Vec[i64] = Vec::new()
    print(average(&empty))
}
```

```console
$ om run main.om
4
panic: average of an empty list
```

A panic prints `panic:` and the message on stderr and exits with code 101.
These also panic:

- `panic("message")`
- `assert(condition)` and `assert(condition, "message")` when false
- `unwrap()` on `None` or `Err`, and `expect("message")`
- `list[i]` when `i` is past the end

A good rule: return `Option` or `Result` when the caller could reasonably
handle the problem; panic when the problem means the program itself is wrong.

## Try it

- Write `fn safe_div(a: i64, b: i64) -> Option[i64]` and use it with
  `unwrap_or`.
- Write `fn sum_all(items: &Vec[str]) -> Result[i64, str]` that parses every
  item with `parse_int()?` and adds them up.

Next: [Collections](08-collections.md).
