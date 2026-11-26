# Om in one page

The whole language as it works today, in one page. Small enough to read in five
minutes or paste into an AI assistant as context (roughly 1,200 tokens).

```om
// ---- basics ----
let x = 5                      // immutable, i64 by default
let mut n: u8 = 0              // mutable, annotated
const MAX: i64 = 100
n += 1                         // + - * / % & | ^ << >> and their = forms
let big = 1_000_000            // also 0xFF 0o17 0b1010 5u8 2.5f32 1e9
let c = 'z'                    // char; also true/false, f32, f64
let t = (1, "a")               // tuple: t.0, t.1, let (a, b) = t
3.9 as i64                     // casts like Rust

// ---- functions ----
fn add(a: i64, b: i64) -> i64 { a + b }           // last expression returns
fn area(w: i64, h: i64 = 1, scale: i64 = 1) -> i64 { w * h * scale }
area(5, scale: 3)                                  // default + named args
3 |> add(4)                                        // pipeline: add(3, 4)
fn main() -> i64 { 0 }                             // optional exit code

// ---- control flow ----
let s = if x > 3 { "big" } else { "small" }        // if is an expression
while n < 10 { n += 1 }
for i in 0..10 { }                                 // also 1..=10, &list, list
let v = loop { break 42 }
'outer: for i in 0..3 { for j in 0..3 { break 'outer } }
defer print("bye")                                 // runs at block exit, newest first

// ---- strings ----
print("sum is {a + b}")        // interpolate any expression; {{ }} for braces
let mut st = String::from("hi")
st += "!"                      // push_str, push, len, contains, find, replace,
st.trim().split_once("=")      // split, lines, chars (return Vec), slice(a, b),
"42".parse_int()               // parse_float, to_uppercase, starts_with ...
fn takes_view(s: str) { }      // str = view; pass a String as &st

// ---- structs, enums, match ----
struct P { x: f64, y: f64 }
impl P {
    fn new(x: f64, y: f64) -> Self { P { x, y } }
    fn len2(&self) -> f64 { self.x * self.x + self.y * self.y }
    fn move_by(&mut self, d: f64) { self.x += d }
}
enum Shape { Circle(f64), Rect(f64, f64), Empty }
match sh {
    Circle(r) => r * r * 3.14,
    Rect(w, h) if w > 0.0 => w * h,
    _ => 0.0,
}                                // must be exhaustive; ranges 1..=9, a | b
if let Some(v) = opt { }        // also while let

// ---- Option, Result, panics ----
fn div(a: i64, b: i64) -> Result[i64, str] { if b == 0 { Err("zero") } else { Ok(a / b) } }
let q = div(1, 2)?              // returns Err early; works on Option too
// unwrap expect unwrap_or is_some is_none ok_or is_ok is_err ok err
panic("bad {x}")                 // assert(c), assert(c, "msg"); exit code 101

// ---- ownership ----
let a = [1, 2, 3]               // Vec[i64]; [0; n] too
let b = a                       // moved: using a is now an error
let c = b.clone()               // deep copy
fn read(v: &Vec[i64]) { }       // borrow
fn grow(v: &mut Vec[i64]) { v.push(1) }
grow(&mut c)                    // refs are parameters only: never stored or returned
impl Drop for P { fn drop(&mut self) { } }   // runs at scope end, reverse order
let bx = Box::new(5)            // *bx; enum List { Cons(i64, Box[List]), Nil }
// Vec: push pop insert remove len get first last contains sort dedup v[i]

// ---- generics and traits ----
fn pick[T](a: T, b: T, f: bool) -> T { if f { a } else { b } }
struct Pair[A, B] { a: A, b: B }
impl[A, B] Pair[A, B] { }
trait Named { fn name(&self) -> str; fn hi(&self) { print("hi {self.name()}") } }
impl Named for P { fn name(&self) -> str { "p" } }
fn show[T: Named + Default](x: T) { }      // or: where T: Named
// built-in traits: Eq Ord (max min clamp) Add Sub Mul Div Rem Neg Default Drop
T::default()                                // call through a type parameter
```

**Not in Om yet:** closures, iterators, `HashMap`, modules/`pub`, `dyn Trait`,
tuple structs, struct patterns, fixed arrays, threads, async, macros, C FFI.
No lifetimes ever: references can't be stored in structs or returned.

**Tools:** `om run f.om`, `om build f.om -o app`, `om check f.om`.
