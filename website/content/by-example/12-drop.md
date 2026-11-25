# Drop

Cleanup runs when a value goes out of scope, in Rust's order. There is no garbage collector.

```om
// Implement Drop to run cleanup when a value goes away. No garbage collector.
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        print("release {self.name}")
    }
}

fn main() {
    let _a = Guard { name: String::from("a") }
    {
        let _b = Guard { name: String::from("b") }
        print("inner scope ends")
    }
    let _c = Guard { name: String::from("c") }
    print("main ends")
    // drops run in reverse order of declaration: c, then a
}
```

Run it:

```console
$ om run 12-drop.om
inner scope ends
release b
main ends
release c
release a
```

The source is in [`12-drop.om`](12-drop.om). Back to [Om by Example](README.md).
