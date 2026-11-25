# Notes from writing The Hollow Bell in Om

A running log, kept while writing and playtesting the game, of everything that
would have made it easier (and cheaper in tokens) for Claude, or any AI, to
write a game like this in Om. Newest entries are added at the end of each
section. Each entry says what happened, what it cost, and a suggested fix.

## Bugs found (and fixed in this branch)

1. **`\u{...}` did not work inside string literals.** The language reference
   lists `\u{...}` and `\'` as string escapes, but the lexer only accepted
   them in char literals. `"\u{1b}[31m"` failed with "unknown escape
   sequence", which blocks all terminal colour. Fixed in `om-lexer`: strings
   now accept `\u{...}`, `\'` and Rust's `\x41` form (up to `\x7F`), with tests.

## Missing features that cost the most

1. **No input at all.** There was no way to read a key, a line, or the time,
   so no interactive program was possible. Added `read_key()`, `time_ns()`
   and `sleep_ms(ms)` to the prelude and the C runtime (raw terminal mode on
   POSIX, `_getch` on Windows). A `read_line() -> Option[String]` and
   `args() -> Vec[String]` are the obvious next two; without `args` the game
   has to ask for a seed on its title screen instead of taking `--seed 42`.
2. **Indexes must be `usize`, but game coordinates are signed.** Every grid
   access needs `as usize` (`self.tiles[(y * W + x) as usize]`), and
   `for o in 0..8` gives `i64`, so `xs[o]` fails until it is written
   `0usize..8`. I wrote an `idx(x, y) -> usize` helper to contain it. Allowing
   any integer type as an index (with the usual bounds check, negative = out
   of range) would remove a whole class of compile errors and tokens.
3. **`Vec::new()` needs a type annotation** even when the very next line
   pushes into it. Rust infers this from later use; Om only infers locally.
   Every `let mut v = Vec::new()` cost a compile round trip.
4. **A call whose value is unused must be wrapped in `let _ =`.** `for _ in
   0..4 { g.rand() }` and `if full { self.log.remove(0) }` are errors ("a loop
   body must not produce a value"). In Rust a trailing `;` discards the value;
   in Om semicolons are optional, so there is no way to say "discard" except
   `let _ =`. Suggestion: allow non-unit values in statement position of loop
   bodies and `if` without `else` (only the block's *tail* in value position
   should matter), or make `;` discard.
5. **Tuples do not implement `Ord`**, so `Vec[(i64, i64)].sort()` fails. I
   packed (distance, index) into one `i64` instead.
6. **No closures yet**, so there is no `sort_by_key`, `retain`, `iter().filter`
   or `any`. Each of these became a hand-written index loop.
7. **No `HashMap`**, so the benchmark's "causes of death" table is two
   parallel `Vec`s searched linearly.
8. **No print without a newline.** A frame is built in one big `String` and
   printed once, which is the right thing for a game anyway, but a
   `write(x)` / `eprint` would help.

## Things that behaved differently from Rust (cost a compile round trip)

1. **`for x in &v` (or over a `&Vec` parameter) yields values, not
   references**, so Rust-style `*x` is an error ("cannot dereference i64").
   32 `*` had to be removed. Either accept `*x` on a copied value as a no-op,
   or say in the error "loop variables over `&Vec[T]` are already values".
2. **A `&mut` parameter must be re-borrowed explicitly when passed on**:
   inside `fn key_to_act(g: &mut Game, ...)`, calling `read_mantra(g)` is an
   error; it has to be `read_mantra(&mut g)`. The error message did say
   exactly what to write, which helped.
3. **String literals are `str`, interpolated strings are `String`**, so
   `if a { "{GREEN}on{RESET}" } else { "off" }` is a type error, and
   `g.prompt = "text"` (a `String` field) needs `.to_string()`. An automatic
   `str` to `String` conversion where a `String` is expected would save many
   round trips.

## What worked well

- Native speed: the balance benchmark plays 100 complete games (about 80,000
  turns of pathfinding, sound flooding and shadowcasting) in about 20 seconds.
- `print` of any struct/enum with no `derive` made debugging easy.
- String interpolation with arbitrary expressions (`"{mon_name(m.kind)} ({dmg})"`)
  kept the message code short.
- Errors point at the exact token, and most say what to write instead.
- Consts of type `str` work, which made ANSI colour constants tidy.
