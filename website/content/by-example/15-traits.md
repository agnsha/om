# Traits and operator overloading

Traits with default methods, trait bounds, and `+` / `==` through `Add` and `Eq`.

```om
// Traits name shared behaviour. Methods with a body are defaults.
trait Animal {
    fn name(&self) -> str
    fn sound(&self) -> str
    fn speak(&self) { print("{self.name()} says {self.sound()}") }
}

struct Dog {}
struct Cat { lives: i64 }

impl Animal for Dog {
    fn name(&self) -> str { "Dog" }
    fn sound(&self) -> str { "woof" }
}

impl Animal for Cat {
    fn name(&self) -> str { "Cat" }
    fn sound(&self) -> str { "meow" }
    fn speak(&self) { print("Cat ({self.lives} lives) says meow") }
}

fn twice[T: Animal](a: T) {
    a.speak()
    a.speak()
}

// Operator overloading through Add and Eq.
struct V2 { x: i64, y: i64 }
impl Add for V2 { fn add(self, o: V2) -> V2 { V2 { x: self.x + o.x, y: self.y + o.y } } }
impl Eq for V2 { fn eq(&self, o: &V2) -> bool { self.x == o.x && self.y == o.y } }

fn main() {
    twice(Dog {})
    Cat { lives: 9 }.speak()
    let v = V2 { x: 1, y: 2 } + V2 { x: 3, y: 4 }
    print(v)
    print(v == V2 { x: 4, y: 6 })
}
```

Run it:

```console
$ om run 15-traits.om
Dog says woof
Dog says woof
Cat (9 lives) says meow
V2 { x: 4, y: 6 }
true
```

The source is in [`15-traits.om`](15-traits.om). Back to [Om by Example](README.md).
