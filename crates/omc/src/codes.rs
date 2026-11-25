//! Stable error codes.
//!
//! Every compiler message maps to a code through the patterns below (`$` matches
//! any text and is captured). Codes never change meaning once released, so
//! tools and agents can key on them; `om explain CODE` prints the long form.
//! A test checks that every message the compiler can produce has a code.

/// One error code.
pub struct Code {
    pub code: &'static str,
    /// A few words, shown by `om explain` and `om guide`.
    pub title: &'static str,
    /// Message patterns; `$` matches any text.
    pub patterns: &'static [&'static str],
    /// What it means and how to fix it, for `om explain`.
    pub explain: &'static str,
}

/// The code for messages no pattern matches. A test keeps this unused.
pub const UNKNOWN: &str = "E0000";

/// All codes, most specific patterns first within each code.
pub const CODES: &[Code] = &[
    // ---------- E01xx: syntax ----------
    Code {
        code: "E0106",
        title: "macro call syntax",
        patterns: &["Om has no macros$"],
        explain: "Om has no macros. `println!(\"{}\", x)` is `print(\"{x}\")`; other `name!(...)` calls are plain function calls. `om fix` rewrites `println!` and `print!` automatically.",
    },
    Code {
        code: "E0107",
        title: "attribute syntax",
        patterns: &["Om has no attributes$"],
        explain: "Om has no `#[...]` attributes. Every value can be printed and cloned, and compared with `==` when it implements `Eq`, so `#[derive(...)]` is unnecessary. Tests are functions named `test_*`. Delete the attribute line.",
    },
    Code {
        code: "E0109",
        title: "Rust keyword Om does not have",
        patterns: &["Om has no `$`$"],
        explain: "Om leaves out some Rust keywords: `pub` (everything in a file is visible), `use` and `mod` (the prelude is always available and a program is one file), `unsafe`, and `impl Trait` in argument position. Delete the keyword; `om fix` does this automatically.",
    },
    Code {
        code: "E0108",
        title: "nesting too deep",
        patterns: &["this nests too deeply$"],
        explain: "Expressions, blocks, types and patterns may nest at most 256 levels, which keeps the compiler from overflowing its stack on hostile input. Real code never comes close: split the expression with `let` bindings or helper functions.",
    },
    Code {
        code: "E0101",
        title: "unterminated literal or comment",
        patterns: &[
            "unterminated string literal",
            "unterminated character literal",
            "unterminated block comment",
            "unterminated `{` in string interpolation$",
        ],
        explain: "A string, character, `/* */` comment or `{...}` interpolation was not closed. Close it on the same line; strings cannot be nested inside `{...}`.",
    },
    Code {
        code: "E0102",
        title: "bad escape",
        patterns: &["unknown escape sequence", "invalid unicode escape$"],
        explain: "Escapes are `\\n \\t \\r \\0 \\\\ \\\" \\'` and `\\u{1F600}`. Write `{{` and `}}` for literal braces in strings.",
    },
    Code {
        code: "E0103",
        title: "unexpected character",
        patterns: &["unexpected character `$`"],
        explain: "This character is not part of Om's syntax. Om source is ASCII outside strings, characters and comments.",
    },
    Code {
        code: "E0104",
        title: "bad number literal",
        patterns: &[
            "integer literal `$` does not fit in 64 bits",
            "literal `$` does not fit in $",
            "pattern value `$` does not fit in $",
            "missing digits after the number prefix",
            "invalid suffix `$` on a number",
        ],
        explain: "A number literal is out of range for its type or badly formed. Suffixes are the type names: `255u8`, `1.5f32`. Prefixes are `0x`, `0o` and `0b`.",
    },
    Code {
        code: "E0105",
        title: "stray brace in string",
        patterns: &["unmatched `}` in string$", "empty `{}` in string$"],
        explain: "In strings, `{expr}` interpolates a value, so literal braces are written `{{` and `}}`. `om fix` doubles them for you.",
    },
    Code {
        code: "E0110",
        title: "bad pipeline",
        patterns: &["the right side of `|>` must be a function name or call"],
        explain: "`x |> f(a)` means `f(x, a)` and `x |> f` means `f(x)`. The right side must name a function.",
    },
    Code {
        code: "E0111",
        title: "misplaced trait bound",
        patterns: &["put trait bounds on the `impl` or `fn` that needs them$", "expected a trait name before `for`"],
        explain: "Bounds go where they are needed: `fn show[T: Show](x: T)` or `impl[T: Eq] Eq for Pair[T]`, never on a struct or enum definition. `impl Trait for Type` needs a plain trait name.",
    },
    Code {
        code: "E0100",
        title: "syntax error",
        patterns: &["expected $, found $", "a range loop binds one name$"],
        explain: "The parser expected something else here. Semicolons are optional, generics use `[T]`, and struct literals are not allowed directly in `if`/`while`/`match` headers (wrap them in parentheses).",
    },
    // ---------- E02xx: names ----------
    Code {
        code: "E0200",
        title: "unknown variable",
        patterns: &["cannot find variable `$`"],
        explain: "No variable, parameter or constant has this name here. Check the spelling (the error lists close matches) and that the `let` comes before this line in the same or an enclosing block.",
    },
    Code {
        code: "E0201",
        title: "unknown function",
        patterns: &["cannot find function `$`"],
        explain: "No function has this name. Built-ins are `print`, `panic` and `assert`; run `om doc` to list the prelude. Om has no `println`, `format` or `vec`.",
    },
    Code {
        code: "E0202",
        title: "unknown type",
        patterns: &["cannot find type `$`"],
        explain: "No type has this name. Om's types are `i8 i16 i32 i64 isize u8 u16 u32 u64 usize f32 f64 bool char str String`, `Vec[T]`, `Box[T]`, `Option[T]`, `Result[T, E]`, tuples, and your structs and enums. Rust's `&str` is `str`, and generics use `[]`: `Vec[i64]`.",
    },
    Code {
        code: "E0203",
        title: "unknown struct",
        patterns: &["cannot find struct `$`", "cannot find struct or enum `$` for this `impl`"],
        explain: "No struct or enum has this name. Define it in this file; Om has no imports.",
    },
    Code {
        code: "E0204",
        title: "unknown trait",
        patterns: &["cannot find trait `$`"],
        explain: "No trait has this name. The prelude defines `Eq Ord Add Sub Mul Div Rem Neg Default`. Om has no `Display`, `Debug`, `Clone` or `Copy`: every value prints and copies already.",
    },
    Code {
        code: "E0205",
        title: "unknown method",
        patterns: &["$ has no method named `$`$", "`$` has no method or variant named `$`", "$ has no methods"],
        explain: "The type has no method with this name. `om doc Type` lists its real methods; the error lists close matches. Rust conversions such as `.to_owned()`, `.as_ref()` and `.into()` do not exist; `.clone()` and `.to_string()` do.",
    },
    Code {
        code: "E0206",
        title: "unknown field",
        patterns: &["struct `$` has no field `$`", "$ has no field `$`"],
        explain: "The struct has no field with this name, or the value is not a struct or tuple. Tuple fields are `.0`, `.1`, ...",
    },
    Code {
        code: "E0207",
        title: "unknown variant",
        patterns: &["`$` has no variant `$`", "expected a variant of `$`, found `$`"],
        explain: "The enum has no variant with this name. Write variants as `Enum::Variant`; `Some`, `None`, `Ok` and `Err` need no prefix.",
    },
    Code {
        code: "E0208",
        title: "function used as a value",
        patterns: &["`$` is a function; call it with `$(...)`"],
        explain: "Functions are not values yet. Call it with parentheses.",
    },
    Code {
        code: "E0209",
        title: "prelude internal",
        patterns: &["`$` is internal to the prelude$"],
        explain: "`intrinsic::*` operations are reserved for the prelude, which wraps them in checked public methods like `wrapping_add` and `sqrt`. Call those instead.",
    },
    Code {
        code: "E0210",
        title: "no main",
        patterns: &["program has no `fn main()`"],
        explain: "Every program needs `fn main() { ... }`, optionally returning an `i64` exit code.",
    },
    Code {
        code: "E0211",
        title: "bad loop label",
        patterns: &["no enclosing loop is labelled `$`", "label `$` is already used by an enclosing loop"],
        explain: "`break 'name` and `continue 'name` need an enclosing loop written `'name: loop { ... }`, and labels must be unique within a nest of loops.",
    },
    // ---------- E03xx: types ----------
    Code {
        code: "E0300",
        title: "mismatched operand types",
        patterns: &["mismatched types: $"],
        explain: "Both sides of an operator must have the same type; Om never converts numbers implicitly. Convert one side with `as`, e.g. `x as f64 + y`.",
    },
    Code {
        code: "E0302",
        title: "wrong return type",
        patterns: &[
            "`$` must return $, but its body ends with $",
            "`$` returns nothing, but its body ends with a $ value",
            "this function returns $, but `return` gives $",
        ],
        explain: "The value a function produces must match its declared return type. A function without `-> T` returns nothing, so end its body with a statement (add `;`) or declare the type.",
    },
    Code {
        code: "E0303",
        title: "needs a bool",
        patterns: &["condition must be bool, found $", "`!` needs a bool$", "`$` needs bools, found $"],
        explain: "Conditions and `&&`/`||` need `bool`. Om has no truthiness: write `x != 0` instead of `x`.",
    },
    Code {
        code: "E0304",
        title: "operator not defined",
        patterns: &[
            "operator `$` is not defined for $",
            "`$` is not defined for bool",
            "cannot negate an unsigned $",
            "cannot negate $; implement `Neg` for it",
        ],
        explain: "This type has no such operator. For your own types implement the trait: `Add` for `+`, `Eq` for `==`, `Ord` for `<`, `Neg` for unary `-`.",
    },
    Code {
        code: "E0305",
        title: "invalid cast",
        patterns: &["cannot cast $ to $"],
        explain: "`as` converts between number types, `bool` and `char` to integers, `u8` to `char`, and unit-only enums to integers. Nothing else casts.",
    },
    Code {
        code: "E0306",
        title: "cannot infer type",
        patterns: &["cannot infer type `$` for $"],
        explain: "Om could not work out a type parameter. Add a type annotation, e.g. `let x: Option[i64] = None`.",
    },
    Code {
        code: "E0307",
        title: "bad `?`",
        patterns: &[
            "`?` needs an Option or Result, found $",
            "`?` on $ can only be used in a function that returns $",
            "`?` cannot convert the error type $ into $",
        ],
        explain: "`x?` unwraps `Some`/`Ok` or returns the `None`/`Err` early, so the function must return the same kind (`Option` or `Result`) with the same error type.",
    },
    Code {
        code: "E0308",
        title: "unused value in statement position",
        patterns: &[
            "`if` without `else` must not produce a value$",
            "a loop body must not produce a value$",
        ],
        explain: "An `if` without `else`, and a loop body, cannot produce a value. Add `;` after the last expression, or add an `else` branch.",
    },
    Code {
        code: "E0309",
        title: "wrong type arguments",
        patterns: &[
            "`$` does not take type arguments",
            "`$` takes $ type argument(s) but $ were given",
            "`$` is not a type parameter here",
            "`Self` is only available inside an `impl` block",
        ],
        explain: "Generic types take their type arguments in square brackets, e.g. `Option[i64]`, `Result[T, str]`.",
    },
    Code {
        code: "E0310",
        title: "trait bound not met",
        patterns: &["$ does not implement `$`, which `$` requires of `$`"],
        explain: "A generic function's bound is not satisfied. Implement the trait for the type, e.g. `impl Eq for Point { fn eq(&self, other: &Point) -> bool { ... } }`.",
    },
    Code {
        code: "E0311",
        title: "recursive type",
        patterns: &["type `$` contains itself$"],
        explain: "A struct or enum cannot contain itself directly, since values have a fixed size. Heap boxes arrive with the standard library.",
    },
    Code {
        code: "E0312",
        title: "type or instantiation too large",
        patterns: &["type $ is too large$", "type $ nests too deeply$", "too many generic instances$"],
        explain: "A value may flatten into at most 1024 scalars, types may nest 64 levels, and a program may create 20000 generic instances. These limits stop hostile or runaway code (such as a generic function calling itself with a growing type) from hanging the compiler.",
    },
    Code {
        code: "E0313",
        title: "bad range",
        patterns: &["`for` ranges need integer bounds$", "the bounds of this range have different types$"],
        explain: "`for i in a..b` needs integer bounds of the same type.",
    },
    Code {
        code: "E0314",
        title: "interpolation outside print",
        patterns: &["string interpolation can only be used inside `print` and `panic`$"],
        explain: "Runtime strings arrive with the standard library; until then `\"{x}\"` works only as the argument of `print`, `panic` and `assert`.",
    },
    Code {
        code: "E0315",
        title: "non-constant constant",
        patterns: &[
            "a `const` value must be a literal$",
            "constant `$` is declared as $ but its value is $",
            "default values must be constants$",
            "default for `$` is $, but the parameter is $",
        ],
        explain: "`const` values and parameter defaults must be literals of the declared type. No code runs at compile time.",
    },
    Code {
        code: "E0301",
        title: "mismatched types",
        patterns: &[
            "expected $, but the value is $",
            "cannot assign $ to a place of type $",
            "$ have different types: $ and $",
            "$ should be $, found $",
        ],
        explain: "The value has a different type than the one needed here. Om never converts implicitly: use `as` for numbers, or change the annotation.",
    },
    // ---------- E04xx: calls ----------
    Code {
        code: "E0400",
        title: "wrong number of arguments",
        patterns: &[
            "`$` takes $ argument(s) but $ were given",
            "`$` takes $ value(s) but $ were given",
            "`print` takes exactly one argument",
            "`panic` takes at most one argument",
            "`assert` takes a condition and an optional message",
        ],
        explain: "The call passes the wrong number of arguments. `print` takes one value; to print several, interpolate: `print(\"{a} {b}\")`. `om doc NAME` shows the signature.",
    },
    Code {
        code: "E0401",
        title: "missing argument",
        patterns: &["missing argument `$` in call to `$`"],
        explain: "A parameter without a default was not given. Pass it positionally or by name: `f(1, width: 2)`.",
    },
    Code {
        code: "E0402",
        title: "bad named argument",
        patterns: &[
            "`$` has no parameter named `$`",
            "argument `$` is given twice",
            "positional arguments must come before named ones",
            "enum variants do not take named arguments",
            "this built-in does not take named arguments",
        ],
        explain: "Named arguments use parameter names, come after positional ones, and each may appear once.",
    },
    Code {
        code: "E0403",
        title: "not a method",
        patterns: &["`$` has no `self` parameter; call it as `$(...)`"],
        explain: "This is an associated function, not a method. Call it with the type path: `Point::new(1, 2)`.",
    },
    Code {
        code: "E0404",
        title: "main called",
        patterns: &["`main` cannot be called"],
        explain: "`main` is the program's entry point and cannot be called from Om code.",
    },
    Code {
        code: "E0405",
        title: "bad struct literal",
        patterns: &["missing field(s) in `$`: $", "field `$` is given twice"],
        explain: "A struct literal must give every field exactly once. `Point { x, y }` is shorthand for `Point { x: x, y: y }`.",
    },
    // ---------- E05xx: patterns ----------
    Code {
        code: "E0500",
        title: "non-exhaustive match",
        patterns: &["`match` does not cover every case; missing: $"],
        explain: "Every possible value must match some arm. Add the listed arms, or `_ => ...` to cover the rest.",
    },
    Code {
        code: "E0501",
        title: "pattern type mismatch",
        patterns: &["this pattern matches $, but the value is $", "`$` has $ field(s), but the pattern has $"],
        explain: "The pattern's shape must fit the value's type.",
    },
    Code {
        code: "E0502",
        title: "refutable let",
        patterns: &["this `let` pattern might not match$"],
        explain: "`let` patterns must always match. Use `if let Some(x) = value { ... }` or `match`.",
    },
    Code {
        code: "E0503",
        title: "binding in or-pattern",
        patterns: &["`|` patterns cannot bind names yet"],
        explain: "Alternatives in `A | B` patterns cannot bind variables yet; use separate arms.",
    },
    // ---------- E06xx: mutability and references ----------
    Code {
        code: "E0600",
        title: "assign to immutable",
        patterns: &["cannot assign twice to `$`; declare it with `let mut $`"],
        explain: "Variables are immutable unless declared `let mut`. `om fix` adds the `mut`.",
    },
    Code {
        code: "E0601",
        title: "mutate self without &mut self",
        patterns: &["cannot change `self` here; take `&mut self` to modify it"],
        explain: "A method that changes `self` must take `&mut self`.",
    },
    Code {
        code: "E0602",
        title: "bad &mut argument",
        patterns: &[
            "cannot pass `$` as `&mut`; $",
            "`&mut` needs a variable or field to change",
            "cannot pass `$` as `&mut` twice in one call",
        ],
        explain: "`&mut` arguments must be mutable variables or fields (`let mut x`), each passed once per call.",
    },
    Code {
        code: "E0603",
        title: "missing &mut",
        patterns: &["$ is `&mut $`; pass it as `&mut ...`"],
        explain: "The parameter is `&mut T`, so the call must say `&mut x`, which makes the change visible at the call site. `om fix` adds it.",
    },
    Code {
        code: "E0604",
        title: "missing &",
        patterns: &["$ is `&$`; pass it as `&...`"],
        explain: "The parameter is `&T`, so the call must say `&x`. `om fix` adds it.",
    },
    Code {
        code: "E0605",
        title: "unexpected reference",
        patterns: &["$ takes a value, not a reference", "`&` is only used when passing an argument$"],
        explain: "Om has no reference values: `&` and `&mut` appear only when passing an argument to a `&` or `&mut` parameter. Remove the `&`.",
    },
    Code {
        code: "E0606",
        title: "bad assignment target",
        patterns: &["can only assign to a variable or a field", "can only assign to $"],
        explain: "The left side of `=` must be a variable, a field, a list element or `*box`, like `x`, `p.x`, `v[i]` or `*b`.",
    },
    // ---------- E07xx: items ----------
    Code {
        code: "E0700",
        title: "defined twice",
        patterns: &[
            "function `$` is defined twice",
            "type `$` is defined twice",
            "trait `$` is defined twice",
            "constant `$` is defined twice",
            "field `$` is declared twice",
            "variant `$` is declared twice",
            "parameter `$` is declared twice",
            "trait `$` declares `$` twice",
            "`$` is implemented twice for `$`",
        ],
        explain: "Each name may be defined once in its scope. Om has no overloading; rename one of them.",
    },
    Code {
        code: "E0701",
        title: "built-in name reused",
        patterns: &["`$` is a built-in function and cannot be redefined", "`$` is the name of a built-in function"],
        explain: "`print`, `panic` and `assert` are built in; pick another name.",
    },
    Code {
        code: "E0702",
        title: "trait method missing",
        patterns: &["missing method `$` required by trait `$`"],
        explain: "An `impl Trait for Type` must define every trait method that has no default body. `om doc Trait` shows the signatures.",
    },
    Code {
        code: "E0703",
        title: "not a trait method",
        patterns: &["`$` is not a method of trait `$`"],
        explain: "Only the trait's own methods may appear in `impl Trait for Type`. Put other methods in a plain `impl Type` block.",
    },
    Code {
        code: "E0704",
        title: "trait signature mismatch",
        patterns: &["`$` does not match trait `$`: expected `$`, found `$`"],
        explain: "A trait method's implementation must have exactly the trait's signature, with `Self` replaced by the type.",
    },
    Code {
        code: "E0705",
        title: "bad impl target",
        patterns: &["this impl must be written `$`", "cannot add methods to built-in type `$`$"],
        explain: "A generic type's impl repeats its parameters: `impl[T] Stack[T] { ... }`. Built-in types get methods only through traits.",
    },
    Code {
        code: "E0706",
        title: "bad main signature",
        patterns: &["`main` takes no parameters", "`main` must return nothing or an i64 exit code"],
        explain: "Write `fn main()` or `fn main() -> i64`, whose value becomes the exit code.",
    },
    Code {
        code: "E0707",
        title: "misplaced self",
        patterns: &["`self` must be the first parameter", "`self` is only allowed in methods inside an `impl` block"],
        explain: "`self`, `&self` or `&mut self` must be the first parameter of a method in an `impl` or `trait` block.",
    },
    Code {
        code: "E0708",
        title: "default order",
        patterns: &["`$` needs a default because an earlier parameter has one"],
        explain: "Once a parameter has a default, every later one needs one too.",
    },
    Code {
        code: "E0709",
        title: "empty enum",
        patterns: &["enum `$` needs at least one variant"],
        explain: "An enum needs at least one variant.",
    },
    // ---------- E08xx: control flow ----------
    Code {
        code: "E0800",
        title: "break or continue outside a loop",
        patterns: &["`$` outside of a loop"],
        explain: "`break` and `continue` only work inside `loop`, `while` and `for`.",
    },
    Code {
        code: "E0801",
        title: "break with value",
        patterns: &["only `loop` can break with a value$"],
        explain: "Only `loop` produces a value: `let x = loop { break 5 }`.",
    },
    Code {
        code: "E0802",
        title: "control flow in defer",
        patterns: &["`return`, `break`, `continue` and `?` are not allowed inside `defer`"],
        explain: "A `defer` body runs when its block exits, so it cannot leave that block itself.",
    },
    // ---------- E09xx: lints ----------
    Code {
        code: "E0900",
        title: "bare unwrap",
        patterns: &["bare `unwrap()` $"],
        explain: "Outside `test_*` functions, `unwrap()` is an error: a panic with no context is how outages happen. Use `?` to pass the failure up, `unwrap_or(default)`, `match`, or `expect(\"why this cannot fail\")`.",
    },
    Code {
        code: "E0901",
        title: "undeclared effect",
        patterns: &["`$` declares `uses $` but $"],
        explain: "A function with a `uses` clause may only perform the effects it lists. `io` covers printing; `panic` covers `panic`, `assert`, `expect` and `unwrap`. Add the effect to the clause or remove the call. `om audit` shows every function's effects.",
    },
    Code {
        code: "E0902",
        title: "unknown effect",
        patterns: &["unknown effect `$`$"],
        explain: "Effects are `io` and `panic`.",
    },
    // ---------- W01xx: warnings ----------
    Code {
        code: "W0100",
        title: "unchecked generic function",
        patterns: &["generic function `$` is never called, so its body was not type-checked"],
        explain: "Generic functions are checked when they are instantiated. One that is never called may hide type errors; call it, add a test, or delete it.",
    },
    // ---------- E10xx: ownership and borrowing ----------
    Code {
        code: "E1000",
        title: "use after move",
        patterns: &["use of moved value `$`$", "`$` is moved inside this loop$"],
        explain: "`String`, `Vec`, `Box` and values that contain them have one owner. Assigning or passing one moves it, and the old name cannot be used again. Pass `&value` to lend it, or call `.clone()` before the move when you need two copies.",
    },
    Code {
        code: "E1001",
        title: "cannot move out of a borrow",
        patterns: &[
            "cannot move out of `$`, which is borrowed$",
            "cannot move out of `&mut` parameter `$`$",
            "cannot move out of this box$",
            "cannot move field `$` out of `$`$",
            "cannot move a field out of $, which implements Drop",
            "cannot move an element out of a list$",
            "cannot move parts out of this place$",
        ],
        explain: "Only the owner can give a value away. Clone it (`.clone()`), take it out with a method such as `remove`, `swap_remove` or `pop`, or borrow it with `&`.",
    },
    Code {
        code: "E1002",
        title: "borrow conflict",
        patterns: &[
            "cannot move `$` while $ borrows it",
            "cannot change `$` while $ borrows it",
            "cannot pass `$` as `&mut` while also passing a view of its text",
        ],
        explain: "While a `str` view or `&` reference points into a value, the value cannot be moved or changed. Finish using the view first, or copy what you need with `.to_string()` or `.clone()`.",
    },
    Code {
        code: "E1003",
        title: "str view outlives its String",
        patterns: &[
            "this `str` points into a temporary String$",
            "cannot return a `str` that points into$",
            "this block's `str` points into a temporary String$",
            "cannot store a `str` view of a String$",
            "`$` could keep a `str` view of a String past its lifetime$",
        ],
        explain: "A `str` is a view of text owned by someone else. A view of a `String` must not outlive it: keep the String in a variable, or turn the view into its own String with `.to_string()`.",
    },
    Code {
        code: "E1004",
        title: "private fields",
        patterns: &["the fields of `$` are private$", "`$` has private fields$"],
        explain: "`Vec`, `String` and `Box` keep their fields private so their memory stays valid. Use their methods (`om doc Vec`).",
    },
    Code {
        code: "E1005",
        title: "list indexing and literals",
        patterns: &[
            "$ cannot be indexed with `[]`",
            "list indexes must be usize, found $",
            "list elements must all have the same type: $",
            "cannot infer the element type of `[]`$",
            "the count in `[value; count]` must be usize, found $",
        ],
        explain: "`v[i]` indexes a `Vec` with a `usize` and panics when `i` is out of range; `v.get(i)` returns an Option instead. List literals `[a, b]` need elements of one type, and `[]` needs a type annotation.",
    },
    Code {
        code: "E1006",
        title: "bad dereference",
        patterns: &["cannot dereference $"],
        explain: "Only a `Box` can be dereferenced with `*`.",
    },
    Code {
        code: "E1007",
        title: "clone of a Drop type",
        patterns: &["`$` implements Drop, so it needs its own `Clone` impl$"],
        explain: "A type with a `Drop` impl manages a resource, so copying its fields would free it twice. Write `impl Clone for T { fn clone(&self) -> Self { ... } }`.",
    },
    // ---------- E09xx: internal ----------
    Code {
        code: "E0999",
        title: "internal compiler error",
        patterns: &[
            "codegen for `$`: $",
            "codegen for $ of $: $",
            "`Ptr` takes one type argument",
            "intrinsic `$` needs a Ptr, found $",
            "intrinsic `$` is missing an argument",
            "intrinsic `$` needs a known Ptr type",
            "ptr_write of $ into $",
            "unknown intrinsic `$`",
            "prelude line $",
            "writing object file: $",
            "unsupported host: $",
            "intrinsic `$` needs an argument",
            "unknown intrinsic `$` for $",
            "linking failed$",
            "could not run the linker$",
            "writing $: $",
            "creating $: $",
            "internal error$",
        ],
        explain: "Something failed inside the compiler or the system linker. Please report it with the program that triggers it.",
    },
];

/// Match `msg` against `pattern`; returns the texts the `$`s matched.
pub fn match_pattern(pattern: &str, msg: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = pattern.split('$').collect();
    if parts.len() == 1 {
        return (pattern == msg).then(Vec::new);
    }
    let mut rest = msg.strip_prefix(parts[0])?;
    let mut caps = Vec::new();
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate().skip(1) {
        if i == last {
            if part.is_empty() {
                caps.push(rest.to_string());
            } else {
                let cap = rest.strip_suffix(part)?;
                caps.push(cap.to_string());
            }
            return Some(caps);
        }
        let at = rest.find(part)?;
        caps.push(rest[..at].to_string());
        rest = &rest[at + part.len()..];
    }
    None
}

/// The code for a compiler message, and the text its `$`s matched.
pub fn classify(msg: &str) -> (&'static Code, Vec<String>) {
    for c in CODES {
        for p in c.patterns {
            if let Some(caps) = match_pattern(p, msg) {
                return (c, caps);
            }
        }
    }
    (&UNKNOWN_CODE, vec![])
}

static UNKNOWN_CODE: Code = Code {
    code: UNKNOWN,
    title: "uncategorized",
    patterns: &[],
    explain: "This message has no code yet. Please report it.",
};

/// Look up a code by name, case-insensitively.
pub fn find(code: &str) -> Option<&'static Code> {
    CODES.iter().find(|c| c.code.eq_ignore_ascii_case(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_capture() {
        assert_eq!(match_pattern("cannot find variable `$`", "cannot find variable `x`"), Some(vec!["x".into()]));
        assert_eq!(
            match_pattern("expected $, found $", "expected `)`, found `{`"),
            Some(vec!["`)`".into(), "`{`".into()])
        );
        assert_eq!(match_pattern("unterminated block comment", "unterminated block comment"), Some(vec![]));
        assert_eq!(match_pattern("a$", "ab"), Some(vec!["b".into()]));
        assert_eq!(match_pattern("cannot find variable `$`", "cannot find function `x`"), None);
    }

    #[test]
    fn codes_are_unique_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for c in CODES {
            assert!(seen.insert(c.code), "duplicate code {}", c.code);
            assert!(c.code.len() == 5 && (c.code.starts_with('E') || c.code.starts_with('W')), "{}", c.code);
            assert!(!c.patterns.is_empty() && !c.explain.is_empty(), "{}", c.code);
        }
    }

    /// Every message template in the compiler's source must map to a real code,
    /// so new errors cannot ship without one. Placeholders become `$`.
    #[test]
    fn every_compiler_message_has_a_code() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut missing = Vec::new();
        let mut checked = 0;
        for file in ["om-lexer/src/lib.rs", "om-parser/src/lib.rs", "om-codegen/src/lib.rs", "omc/src/lib.rs"] {
            let src = std::fs::read_to_string(root.join(file)).unwrap();
            for (msg, is_expected) in templates(&src) {
                let msg = if is_expected { format!("expected {msg}, found $") } else { msg };
                checked += 1;
                if classify(&msg).0.code == UNKNOWN {
                    missing.push(format!("{file}: {msg}"));
                }
            }
        }
        assert!(checked > 100, "found only {checked} messages; is the scanner broken?");
        assert!(
            missing.is_empty(),
            "messages without an error code (add them to omc/src/codes.rs):\n{}",
            missing.join("\n")
        );
    }

    /// String literals passed to the compiler's error helpers, with `{..}` placeholders as `$`.
    fn templates(src: &str) -> Vec<(String, bool)> {
        let mut out = Vec::new();
        for marker in ["err(", ".error(", "message:", "internal(", "Diagnostic::plain("] {
            let mut from = 0;
            while let Some(i) = src[from..].find(marker) {
                let at = from + i + marker.len();
                from = at;
                let rest = src[at..].trim_start();
                let formatted = rest.starts_with("format!(");
                let rest = rest.strip_prefix("format!(").map_or(rest, str::trim_start);
                let Some(body) = rest.strip_prefix('"') else { continue };
                let mut lit = String::new();
                let mut chars = body.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some(n) = chars.next() {
                                lit.push(n);
                            }
                        }
                        '"' => break,
                        c => lit.push(c),
                    }
                }
                // Only `format!` treats braces as placeholders.
                let lit = if formatted { placeholders(&lit) } else { lit };
                out.push((lit, marker == ".error("));
            }
        }
        out
    }

    fn placeholders(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    out.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    out.push('}');
                }
                '{' => {
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                    out.push('$');
                }
                c => out.push(c),
            }
        }
        out
    }
}
