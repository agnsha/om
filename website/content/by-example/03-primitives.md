# Primitive types

Every integer type from `i8` to `u64`, `isize`/`usize`, `f32`, `f64`, `bool` and `char`, with Rust's limits, number methods and `as` casts.

```om
// Integers from i8 to u64, floats, bool and char, with Rust's methods and limits.
fn main() {
    let small: u8 = 200
    let big = 1_000_000i64
    let hex = 0xFF
    let bits = 0b1010
    print("{small} {big} {hex} {bits}")

    print(u8::MAX)
    print(i64::MIN)
    print(f64::EPSILON)

    print(2.0_f64.sqrt())
    print((-7i64).rem_euclid(3))
    print(17u32.count_ones())
    print(250u8.checked_add(10))
    print(250u8.saturating_add(10))

    let c = 'z'
    print(c.is_ascii_alphabetic())
    print(c.to_ascii_uppercase())
    print('7'.to_digit(10))

    // `as` converts between number types, like Rust.
    print(3.99 as i64)
    let wide = 300
    print(wide as u8)
    print(65u8 as char)
}
```

Run it:

```console
$ om run 03-primitives.om
200 1000000 255 10
255
-9223372036854775808
0.0000000000000002220446049250313
1.4142135623730951
2
2
None
255
true
Z
Some(7)
3
44
A
```

The source is in [`03-primitives.om`](03-primitives.om). Back to [Om by Example](README.md).
