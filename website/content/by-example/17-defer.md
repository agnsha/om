# defer

An Om extra from Zig, Go and Swift: put cleanup right next to setup.

```om
// `defer` runs code when the block exits, newest first, even on early return (Om extra).
fn process(n: i64) -> i64 {
    print("open {n}")
    defer print("close {n}")
    if n < 0 { return 0 }
    defer print("done with {n}")
    n * 10
}

fn main() {
    print(process(4))
    print(process(-1))
}
```

Run it:

```console
$ om run 17-defer.om
open 4
done with 4
close 4
40
open -1
close -1
0
```

The source is in [`17-defer.om`](17-defer.om). Back to [Om by Example](README.md).
