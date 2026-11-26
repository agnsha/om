# Control flow

`if` is an expression. `while`, `for` over ranges, `loop` with `break value`, and loop labels.

```om
// `if` and blocks are expressions. Loops: while, loop, for.
fn main() {
    let n = 42
    let size = if n > 100 { "big" } else if n > 10 { "medium" } else { "small" }
    print(size)

    let mut i = 0
    while i < 3 {
        print("while {i}")
        i += 1
    }

    for k in 1..=3 {
        print("for {k}")
    }

    // `loop` can hand back a value with `break`.
    let mut x = 1
    let first = loop {
        x *= 3
        if x > 50 { break x }
    }
    print("first power of 3 over 50: {first}")

    // Labels let `break` and `continue` reach an outer loop.
    'outer: for a in 1..10 {
        for b in 1..10 {
            if a * b == 12 {
                print("{a} * {b} = 12")
                break 'outer
            }
        }
    }
}
```

Run it:

```console
$ om run 05-control-flow.om
medium
while 0
while 1
while 2
for 1
for 2
for 3
first power of 3 over 50: 81
2 * 6 = 12
```

The source is in [`05-control-flow.om`](05-control-flow.om). Back to [Om by Example](README.md).
