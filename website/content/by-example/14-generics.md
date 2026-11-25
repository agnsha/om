# Generics

Generic functions, structs and `impl` blocks. Om compiles a specialised copy for each type used, the same way Rust does.

```om
// Type parameters go in square brackets. Each use gets its own fast copy.
struct Pair[A, B] {
    first: A,
    second: B,
}

impl[A, B] Pair[A, B] {
    fn swap(self) -> Pair[B, A] {
        Pair { first: self.second, second: self.first }
    }
}

fn pick[T](a: T, b: T, first: bool) -> T {
    if first { a } else { b }
}

fn largest[T: Ord](a: T, b: T) -> T {
    a.max(b)
}

fn main() {
    let p = Pair { first: 1, second: "one" }
    print(p.swap())
    print(pick(3.5, 7.0, false))
    print(pick("left", "right", true))
    print(largest(4, 9))
    let none: Option[i64] = None
    print(none)
}
```

Run it:

```console
$ om run 14-generics.om
Pair { first: "one", second: 1 }
7
left
9
None
```

The source is in [`14-generics.om`](14-generics.om). Back to [Om by Example](README.md).
