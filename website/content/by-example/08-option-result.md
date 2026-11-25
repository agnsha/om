# Option, Result and ?

No null and no exceptions. `?` returns the failure from the current function.

```om
// Option[T] and Result[T, E] replace null and exceptions. `?` passes failures up.
fn find_even(list: &Vec[i64]) -> Option[i64] {
    for x in list {
        if x % 2 == 0 { return Some(x) }
    }
    None
}

fn divide(a: i64, b: i64) -> Result[i64, str] {
    if b == 0 { Err("division by zero") } else { Ok(a / b) }
}

fn average(total: i64, count: i64) -> Result[i64, str] {
    let q = divide(total, count)?     // returns early on Err
    Ok(q)
}

fn main() {
    let nums = [3, 5, 8, 9]
    print(find_even(&nums))
    print(find_even(&[1, 3]))

    if let Some(e) = find_even(&nums) {
        print("first even: {e}")
    }

    print(average(10, 2))
    print(average(10, 0))
    print(divide(9, 3).unwrap())
    print(divide(1, 0).unwrap_or(-1))

    match "42".parse_int() {
        Ok(n) => print("parsed {n}"),
        Err(e) => print("bad number: {e}"),
    }
}
```

Run it:

```console
$ om run 08-option-result.om
Some(8)
None
first even: 8
Ok(5)
Err("division by zero")
3
-1
parsed 42
```

The source is in [`08-option-result.om`](08-option-result.om). Back to [Om by Example](README.md).
