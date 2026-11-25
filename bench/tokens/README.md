# Token budget

The same small programs written idiomatically in Om, Rust and Python. The test
`crates/omc/tests/token_budget.rs` checks that every Om program compiles, prints
what its `.expected` file says (the Rust version prints the same), uses no more
tokens than the Rust version, and that Om stays within 90% of Rust and 110% of
Python over all tasks.

Count a file yourself with `om tokens file`. The count is an approximation of a
byte-pair tokenizer, good for comparing languages, not for billing.

| Task | Om | Rust | Python |
|---|---|---|---|
| fizzbuzz | 111 | 121 | 80 |
| shapes | 173 | 206 | 162 |
| parse_sum | 118 | 159 | 147 |
| stack | 178 | 211 | 184 |
| words | 127 | 142 | 127 |
| **total** | **707** | **839** | **700** |
