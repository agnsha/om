# Compiler errors

Om stops at the first error and points at the exact spot. These are the ones
you'll see most, with the real message and the usual fix. The messages below
were produced by the current compiler.

## Assigning to an immutable variable

```om
fn main() {
    let x = 1
    x = 2
}
```

```text
error: cannot assign twice to `x`; declare it with `let mut x`
 --> mut.om:3:5
  |
3 |     x = 2
  |     ^^^^^
```

**Fix:** write `let mut x = 1`.

## Using a value after it moved

```om
fn main() {
    let a = [1, 2, 3]
    let b = a
    print(a)
}
```

```text
error: use of moved value `a`: it was moved earlier (clone it before the move if you need it again)
```

A `Vec`, `String` or `Box` has one owner. `let b = a` hands the list to `b`,
so `a` is empty.

**Fix:** `let b = a.clone()` if you need two copies, or pass `&a` to a function
that only reads it.

## A match that misses a case

```om
enum Light { Red, Yellow, Green }

fn main() {
    let l = Light::Red
    match l {
        Red => print("stop"),
        Green => print("go"),
    }
}
```

```text
error: `match` does not cover every case; missing: Light::Yellow (add an arm for it or a `_` arm)
```

**Fix:** add `Yellow => ...`, or `_ => ...` to catch everything else. For
numbers the compiler names the missing range, such as `i64::MIN..=-1`.

## Changing something while it's borrowed

```om
fn main() {
    let mut v = [1, 2]
    let r = &v
    v.push(3)
    print(r)
}
```

```text
error: cannot change `v` while `r` borrows it
```

**Fix:** finish using `r` before changing `v`, for example by putting the
borrow in its own `{ }` block.

## Returning a view of a local String

```om
fn first_word() -> str {
    let s = String::from("hello world")
    s.slice(0, 5)
}
```

```text
error: cannot return a `str` that points into `s`, which is dropped when the function returns; return a String (`.to_string()`) instead
```

A `str` is a view into text someone else owns. Here the owner `s` is freed when
the function ends.

**Fix:** return `String` and write `s.slice(0, 5).to_string()`.

## The same variable as `&mut` twice

```om
fn swap(a: &mut i64, b: &mut i64) {
    let t = a
    a = b
    b = t
}

fn main() {
    let mut x = 1
    swap(&mut x, &mut x)
}
```

```text
error: cannot pass `x` as `&mut` twice in one call
```

**Fix:** pass two different variables.

## Wrong type

```om
fn main() {
    let x: i64 = "hi"
}
```

```text
error: expected i64, but the value is str
```

## A condition that isn't bool

```om
fn main() {
    if 1 { print("yes") }
}
```

```text
error: condition must be bool, found i64
```

Om never treats numbers as true or false. **Fix:** compare, as in `if n != 0`.

## A literal too big for its type

```om
fn main() {
    let x: u8 = 300
}
```

```text
error: literal `300` does not fit in u8 (range 0..=255)
```

## Panics at run time

Some mistakes can only be caught while the program runs. They print `panic:`
and a message on stderr and exit with code 101:

- `v[i]` with `i` past the end of the list
- `unwrap()` on `None` or `Err`, or `expect(...)` with your message
- a failed `assert(cond)` or `assert(cond, "message")`
- an explicit `panic("message")`

Integer division by zero also stops the program, but today it does so with a
machine trap rather than a `panic:` message (`om run` reports "the program was
killed by signal 4" on Linux). Check the divisor first, or use
`checked_div`, which returns `None`.

**In review:** every error will get a stable code (like Rust's `E0382`) and an
`om explain <CODE>` page, and `om fix` will apply the suggested fix for you. See
[Tools](tools.md#in-review).
