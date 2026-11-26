# Vec

A growable list that owns its elements. Indexing past the end panics, as in Rust.

```om
// Vec[T] is a growable list that owns its elements.
fn main() {
    let mut v: Vec[i64] = Vec::new()
    for i in 1..=5 { v.push(i * i) }
    print(v)
    print(v.len())
    print(v[2])
    v[0] = 100
    print(v.pop())
    print(v.first())
    print(v.contains(&9))

    let mut w = [5, 3, 9, 1, 3]
    w.sort()
    w.dedup()
    print(w)

    let zeros = [0; 4]
    print(zeros)

    let mut total = 0
    for x in &w { total += x }
    print("sum = {total}")

    let grid = [[1, 2], [3, 4]]
    print(grid[1][0])
}
```

Run it:

```console
$ om run 09-vec.om
[1, 4, 9, 16, 25]
5
9
Some(25)
Some(100)
true
[1, 3, 5, 9]
[0, 0, 0, 0]
sum = 18
3
```

The source is in [`09-vec.om`](09-vec.om). Back to [Om by Example](README.md).
