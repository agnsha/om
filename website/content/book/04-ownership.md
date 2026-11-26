# 4. Ownership

This is the chapter that makes Om different from most languages, and the same
as Rust. It explains how Om frees memory without a garbage collector and
without you calling `free`.

## The idea

Some values live entirely on the stack: numbers, `bool`, `char`, and structs
and tuples made only of those. Copying them is cheap, so Om just copies them.

Other values own memory on the heap: a `Vec` owns its elements, a `String`
owns its bytes, a `Box` owns its value. For those, Om follows three rules:

1. Each heap value has exactly one **owner**, a variable or a field.
2. When the owner goes out of scope, the value is **dropped** and its memory
   freed.
3. Assigning the value or passing it to a function **moves** it to a new
   owner. The old variable can't be used any more.

```om
fn main() {
    let a = String::from("hello")
    {
        let b = String::from("inner")
        print(b)
    }                          // b is dropped here
    print(a)
}                              // a is dropped here
```

```console
$ om run main.om
inner
hello
```

Nothing is reference-counted and nothing is scanned in the background. The
compiler inserts the frees at exactly the right places.

## Moves

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

After `let b = a`, the list belongs to `b`. If `a` could still be used, both
would try to free the same memory when they go out of scope. Om rules that out
when it compiles.

The same happens with function calls:

```om
fn count(list: Vec[i64]) -> i64 {
    list.len() as i64
}

fn main() {
    let nums = [4, 5, 6]
    print(count(nums))
    // print(nums) here would be an error: count took ownership
}
```

```console
$ om run main.om
3
```

## Clone

If you need two independent copies, say so with `.clone()`:

```om
fn main() {
    let a = [1, 2, 3]
    let mut b = a.clone()
    b.push(4)
    print(a)
    print(b)
}
```

```console
$ om run main.om
[1, 2, 3]
[1, 2, 3, 4]
```

`.clone()` copies everything the value owns. It's explicit on purpose: you can
always see where a program copies memory.

## Borrowing with `&` and `&mut`

Moving a list into every function that wants to look at it would be tiresome.
Instead, a function can **borrow** a value:

```om
fn total(list: &Vec[i64]) -> i64 {
    let mut sum = 0
    for i in 0..list.len() {
        sum += list[i]
    }
    sum
}

fn add_bonus(list: &mut Vec[i64], bonus: i64) {
    list.push(bonus)
}

fn main() {
    let mut scores = [10, 20, 30]
    print(total(&scores))
    add_bonus(&mut scores, 5)
    print(scores)
    print(total(&scores))
}
```

```console
$ om run main.om
60
[10, 20, 30, 5]
65
```

- `&T` is a read-only borrow. The caller writes `&scores`.
- `&mut T` lets the function change the caller's value. The caller writes
  `&mut scores`, and `scores` must be declared `mut`.

Both are visible at the call site, so you can always tell when a function
might change your data.

You can't pass the same variable as `&mut` twice in one call, because the
function would then have two names for one value:

```text
error: cannot pass `x` as `&mut` twice in one call
```

### Borrowing works on numbers too

```om
fn bump(counter: &mut i64) {
    counter += 1
}

fn main() {
    let mut hits = 0
    bump(&mut hits)
    bump(&mut hits)
    print(hits)
}
```

```console
$ om run main.om
2
```

Inside the function, a `&mut i64` is used like a plain `i64`. There's no `*`
to write.

## Why Om needs no lifetimes

In Rust, a reference can be stored in a struct or returned from a function, so
the compiler needs lifetime annotations like `'a` to prove it never outlives
what it points to. Om makes a simpler rule: **references are only
parameters**. You can't keep one in a struct or return one. A `&mut` parameter
works like Swift's `inout`: the value goes in and its new value comes back.

That rule removes lifetimes from the language. The cost is that data
structures hold owned values (a `String`, an index into a list) instead of
references.

## Local borrows

Inside a function you can also borrow with `let r = &x`. While `r` is in
scope, `x` is frozen: it can't be changed or moved.

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

To fix it, finish with `r` first. Wrapping the borrow in its own block is an
easy way:

```om
fn main() {
    let mut v = [1, 2]
    {
        let r = &v
        print(r)
    }
    v.push(3)
    print(v)
}
```

```console
$ om run main.om
[1, 2]
[1, 2, 3]
```

## Drop

When a value is dropped, Om frees its memory. You can also run your own code at
that moment by implementing the `Drop` trait:

```om
struct Connection {
    id: i64,
}

impl Drop for Connection {
    fn drop(&mut self) {
        print("closing connection {self.id}")
    }
}

fn main() {
    let _first = Connection { id: 1 }
    let _second = Connection { id: 2 }
    print("working")
}
```

```console
$ om run main.om
working
closing connection 2
closing connection 1
```

Values are dropped in reverse order of creation, the same order as Rust. This
is how files, sockets and locks will clean up after themselves without a
`close()` you could forget.

## Summary

| You write | What happens |
|---|---|
| `let b = a` | `a` is copied if it owns nothing, otherwise moved to `b` |
| `a.clone()` | A full, independent copy |
| `f(a)` | Moves `a` into `f` (or copies it) |
| `f(&a)` | `f` can read `a` |
| `f(&mut a)` | `f` can change `a` |
| End of scope | Values are dropped, newest first |

Next: [Structs](05-structs.md).
