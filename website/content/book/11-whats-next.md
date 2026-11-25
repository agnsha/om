# 11. What's next

You've now seen all of Om as it exists today. This chapter is honest about the
gaps, shows how to work around them, and says where the language is going.

## Not in Om yet

| Missing | Work around it with |
|---|---|
| Closures (`\|x\| x + 1`) | Named functions, and `\|>` for chains |
| Iterators (`map`, `filter`, `sum`, `collect`) | A `for` loop that pushes into a new `Vec` |
| `HashMap`, `HashSet`, `VecDeque`, `BTreeMap` | A sorted `Vec` of pairs, or a `Vec` of structs |
| Modules, `use`, `pub` | One file per program for now |
| Packages and a package manager | Copy code between files |
| Reading input, files, the network | Put data in the program as literals |
| Fixed-size arrays `[T; N]` and slices | `Vec[T]` |
| Trait objects `dyn Trait` | An enum with one variant per type |
| Tuple structs and unit structs | A struct with named fields, or `struct Name {}` |
| `i128`, `u128` | `i64` / `u64` |
| Threads, async | Not yet possible |
| C interop (`extern`) | Not yet possible |
| Overflow checks | `checked_add`, `saturating_add` and friends |

Here's the iterator workaround, for example. In Rust you'd write
`v.iter().filter(|x| *x % 2 == 0).map(|x| x * x).collect()`:

```om
fn even_squares(v: &Vec[i64]) -> Vec[i64] {
    let mut out: Vec[i64] = Vec::new()
    for i in 0..v.len() {
        let x = v[i]
        if x % 2 == 0 {
            out.push(x * x)
        }
    }
    out
}

fn main() {
    print(even_squares(&[1, 2, 3, 4, 5, 6]))
}
```

```console
$ om run main.om
[4, 16, 36]
```

And an enum in place of `dyn Trait`:

```om
struct Circle { r: f64 }
struct Square { side: f64 }

enum AnyShape {
    C(Circle),
    S(Square),
}

impl AnyShape {
    fn area(&self) -> f64 {
        match self {
            C(c) => 3.14159 * c.r * c.r,
            S(s) => s.side * s.side,
        }
    }
}

fn main() {
    let shapes = [AnyShape::C(Circle { r: 1.0 }), AnyShape::S(Square { side: 2.0 })]
    let mut total = 0.0
    for s in &shapes {
        total += s.area()
    }
    print(total)
}
```

```console
$ om run main.om
7.14159
```

## The roadmap

| Phase | What it adds |
|---|---|
| 3.5 (next) | Closures, iterators, `HashMap`, modules and `pub` |
| 4 | A real standard library: strings, collections, files. **Regions**, C interop |
| 5 | An LLVM backend for optimised release builds, and a public benchmark suite against Rust, C and Go |
| 6 | An HTTP server library with a region per request, and structured concurrency |

**Regions** are the big idea still to come. A region is an arena: every
allocation in it is a pointer bump, and the whole region is freed in one step.
A web server can give each request its own region, so per-request memory costs
almost nothing to allocate and nothing to free one by one. That's where Om aims
to be faster than typical Rust code. Everywhere else, the honest goal is to
match Rust's speed while compiling faster.

## Keep going

- Work through [Om Exercises](../exercises/README.md).
- Read the programs in [Om by Example](../by-example/README.md) and change them.
- Check the [Rust parity checklist](https://github.com/agnsha/om/blob/main/docs/rust-parity.md)
  to see what's landed since this was written.
- Read the [design document](https://github.com/agnsha/om/blob/main/docs/design.md)
  for the reasoning behind Om's choices.
- Found a bug or want a feature? Open an issue on the repository.

Back to [the Om Book](README.md) or [Learn Om](../README.md).
