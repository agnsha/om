# Pipeline operator

An Om extra: `x |> f(y)` means `f(x, y)`.

```om
// x |> f(y) means f(x, y). Chains read left to right (Om extra).
fn double(x: i64) -> i64 { x * 2 }
fn add(x: i64, y: i64) -> i64 { x + y }
fn square(x: i64) -> i64 { x * x }

fn main() {
    let nested = square(add(double(3), 4))
    let piped = 3 |> double |> add(4) |> square
    print(nested)
    print(piped)
}
```

Run it:

```console
$ om run 16-pipeline.om
100
100
```

The source is in [`16-pipeline.om`](16-pipeline.om). Back to [Om by Example](README.md).
