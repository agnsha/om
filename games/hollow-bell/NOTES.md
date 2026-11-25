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

9. **Constants cannot be used as `match` patterns** (`match t { T_WALL => ..`
   is "this pattern matches `T_WALL`, but the value is i64"), and **unit-only
   enums have no `==`** unless you write an `Eq` impl. Together these pushed
   the game to plain `i64` tile and creature codes, and then to `match`
   arms on bare numbers (`0 => "#", 1 => "."`), which are hard to read and
   easy to get wrong. Either fix would make the code much clearer; a
   built-in `==` for enums (like the built-in printing) is the cheapest.
10. **No `eprint`/`dbg`.** To find out why a sleeping monk woke up I made a
    copy of the game whose wake-up paths called `panic("woke by ...")`. A
    `dbg(x)` that prints to stderr with the line number would have been a
    one-line change.
11. **One file per program.** With no modules the game is a single
    2,400-line file. For an AI that edits by search-and-replace that is
    workable, but every edit needs a unique anchor string, and splitting
    rendering, AI and generation into files would make each read cheaper.

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

## Balance log (100-game autopilot benchmark)

Each line is one change and what the benchmark said afterwards. The autopilot
fights far more than a careful player needs to, so a low autopilot win rate is
expected; the aim was "winnable, and deaths mostly from creatures the stealth
rules are about".

| Change | Autopilot wins | Main killers |
|---|---|---|
| First playable version (melee 1-3, OM 3 damage, breath 8) | 1 | shades 40, listeners 38 |
| OM 4 damage, breath 10 and +1 per 3 turns, melee 2-4, shades sing every other turn, listener 11 HP | 59 | shades 24 |
| The mountain groans every 150 turns on a level (wakes sleepers, sends a hunter), resting regains breath less fast, one more creature per level | 36 | shades 27, listeners 17 |
| Herbs 0-2 per level instead of 1-3; the Keeper returns to its vigil after a lure | 19 | listeners 33, shades 22 |
| Autopilot sneaks past listeners instead of fighting them | 16 | listeners 33, shades 28 |

Hand playtests found: a random wake-up could rouse the monster you were
sneaking up on (now creatures only wake at random when you cannot see them);
sneak attacks did too little to matter (now triple damage, and a sleeping
monk dies in one blow); the sidebar overflowed 80 columns once all ten
syllables were known; long messages wrapped and scrolled the screen (now
wrapped to the log area); the chant's own noise was heard before its damage
landed, so killed creatures "stirred" first.

## What worked well

- Native speed: the balance benchmark plays 100 complete games (about 80,000
  turns of pathfinding, sound flooding and shadowcasting) in about 20 seconds.
- `print` of any struct/enum with no `derive` made debugging easy.
- String interpolation with arbitrary expressions (`"{mon_name(m.kind)} ({dmg})"`)
  kept the message code short.
- Errors point at the exact token, and most say what to write instead.
- Consts of type `str` work, which made ANSI colour constants tidy.
