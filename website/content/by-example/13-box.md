# Box and recursive types

`Box[T]` stores one value on the heap, which is how a type can contain itself.

```om
// Box[T] puts one value on the heap. It lets a type contain itself.
enum List {
    Cons(i64, Box[List]),
    Nil,
}

fn sum(l: &List) -> i64 {
    match l {
        Cons(x, rest) => x + sum(&*rest),
        Nil => 0,
    }
}

fn main() {
    let list = List::Cons(1, Box::new(List::Cons(2, Box::new(List::Cons(3, Box::new(List::Nil))))))
    print(sum(&list))

    let mut b = Box::new(10)
    *b += 5
    print(*b)
}
```

Run it:

```console
$ om run 13-box.om
6
15
```

The source is in [`13-box.om`](13-box.om). Back to [Om by Example](README.md).
