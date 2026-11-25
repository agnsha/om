# Om Exercises

Twelve small programs that don't work yet. Your job is to fix each one so that
`om run` prints `ok`. They follow the order of [the Om Book](../book/README.md),
so read a chapter, then do its exercises.

```console
$ om run 01_variables.om
error: cannot assign twice to `count`; declare it with `let mut count`
...
(fix the file)
$ om run 01_variables.om
ok
```

Some exercises don't compile until you fix them; others compile and then fail
an `assert`. Either way, read the message: it usually tells you what to do.

| # | Exercise | Book chapter | Hint |
|---|---|---|---|
| 1 | [`01_variables.om`](01_variables.om) | [3](../book/03-common-concepts.md) | Variables can't change unless you say so. |
| 2 | [`02_functions.om`](02_functions.om) | [3](../book/03-common-concepts.md) | Parameters need types, and so does the return value. |
| 3 | [`03_if.om`](03_if.om) | [3](../book/03-common-concepts.md) | `if` produces a value; both branches must too. |
| 4 | [`04_loops.om`](04_loops.om) | [3](../book/03-common-concepts.md) | `1..=n` includes `n`. |
| 5 | [`05_move.om`](05_move.om) | [4](../book/04-ownership.md) | A function that only reads a list should borrow it. |
| 6 | [`06_borrow_mut.om`](06_borrow_mut.om) | [4](../book/04-ownership.md) | Changing the caller's value needs `&mut` in two places. |
| 7 | [`07_structs.om`](07_structs.om) | [5](../book/05-structs.md) | One method reads `self`, the other changes it. |
| 8 | [`08_match.om`](08_match.om) | [6](../book/06-enums.md) | The error names the missing case. |
| 9 | [`09_option.om`](09_option.om) | [7](../book/07-errors.md) | Return `Some(...)` as soon as you find it. |
| 10 | [`10_result.om`](10_result.om) | [7](../book/07-errors.md) | One character per line. |
| 11 | [`11_strings.om`](11_strings.om) | [8](../book/08-collections.md) | `chars()` returns a `Vec[char]`; take element `[0]`. |
| 12 | [`12_traits.om`](12_traits.om) | [9](../book/09-generics-traits.md) | Two `impl` blocks and one function with a bound. |

Stuck? Every exercise has a worked answer in [`solutions/`](solutions/). Try
for a while before you look.

**Planned:** an `om exercises` runner that checks your progress and moves to the
next exercise automatically, like Rust's Rustlings.

Back to [Learn Om](../README.md).
