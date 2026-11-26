# Structs and methods

Named fields, struct literals with field shorthand, and `impl` blocks with `&self`, `&mut self` and associated functions.

```om
// Structs group named fields. `impl` adds methods and associated functions.
struct Rect {
    w: f64,
    h: f64,
}

impl Rect {
    fn new(w: f64, h: f64) -> Self { Rect { w, h } }
    fn square(side: f64) -> Self { Rect { w: side, h: side } }

    fn area(&self) -> f64 { self.w * self.h }
    fn fits_in(&self, other: &Rect) -> bool { self.w <= other.w && self.h <= other.h }
    fn scale(&mut self, k: f64) {
        self.w *= k
        self.h *= k
    }
}

fn main() {
    let mut r = Rect::new(3.0, 4.0)
    print(r)
    print(r.area())
    r.scale(2.0)
    print(r)
    print(Rect::square(5.0).fits_in(&r))
    r.h = 1.0
    print("now {r.w} by {r.h}")
}
```

Run it:

```console
$ om run 06-structs.om
Rect { w: 3.0, h: 4.0 }
12
Rect { w: 6.0, h: 8.0 }
true
now 6 by 1
```

The source is in [`06-structs.om`](06-structs.om). Back to [Om by Example](README.md).
