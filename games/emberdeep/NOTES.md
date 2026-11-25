# Notes from writing Emberdeep in Om

Kept while writing and playtesting the game, the same way as
[The Hollow Bell's notes](../hollow-bell/NOTES.md): everything about Om that
cost a compile round trip, a workaround or extra tokens, and what would fix it.

## Fixed in the compiler before starting (from The Hollow Bell's notes)

Integer indexes of any type, `Vec::new()` typed by a later `push`, unused
values dropped in statement position, `str` becoming `String` where one is
expected, `==` on tuples, structs and unit enums without an `Eq` impl,
constants as `match` patterns, `*x` on a plain value, `eprint`, `print(x, end:)`,
`args()`, `read_line()`, `poll_key()` (keys without waiting, for real-time
games), `key_down(k)` (held keys, on Windows), `term_cols()`/`term_rows()`, and
`flush()`. Every one of these was used by this game; without `poll_key` and the
terminal size it could not have been written at all.

## Found while writing this game, and fixed in this branch

1. **A `(` at the start of a line was read as a call of the line before.**
   ```om
   let w3 = fx * fy
   (a.0 * w0 + ..., ...)      // read as fy(a.0 * w0 + ...)
   ```
   The error was "cannot find function `fy`", which is very confusing. `[` on a
   new line already started a new expression; now `(` does too (Go, Swift and
   Kotlin have the same rule).
2. **Comparing integers of different types was an error**, and the most common
   case is `while head < self.queue.len()` with `head: i64`. Comparisons now
   work by value across integer types (a negative number is less than any
   unsigned one); arithmetic still asks for `as`.
3. **Big structs had to list every field twice**: once in the declaration and
   once in the constructor. The game's state struct has 80 fields, so every new
   field meant editing two places 1,500 lines apart, and the constructor was a
   wall of `x: 0.0, y: 0.0, ...`. Struct fields can now have defaults
   (`oil: f64 = OIL_MAX`), and the constructor shrank to the four fields that
   depend on the seed.
4. **Ctrl+C left the terminal broken**: a game that hides the cursor and uses
   the alternate screen had no way to restore them when interrupted. Once a
   program reads keys, the runtime now restores the terminal on Ctrl+C before
   stopping (on Linux, macOS and Windows).

## Still missing (worked around in the game)

1. **Borrowing a list in a `for` loop borrows all of `self`.** Inside
   `for e in &self.ents { ... }` the game cannot write `self.fb[i]`, call a
   `&mut self` method, or even set a flag on `self`. Each case became an index
   loop (`for i in 0..self.ents.len() { let e = self.ents[i] ... }`), which
   works because `Ent` is plain data and is copied. Tracking borrows per field
   (as Rust does for direct field access) would remove this.
2. **No fixed-size arrays.** `[1, -1, 0, 0][d]` builds a heap `Vec` every time
   it runs, so inside a breadth-first search it allocates four times per cell.
   The game uses `dx4(d)` / `dy4(d)` helper functions instead. Arrays on the
   stack (`[i64; 4]`) or constant arrays would be the fix.
3. **No way to format a number into an existing String.** The renderer writes
   about 10,000 colour codes a frame; `out.push_str("{r}")` would allocate a
   String per number, so the game has its own `push_num`. A `push_int` (or a
   `write!`-like form that appends) belongs in the standard library.
4. **A literal takes the other side's type in a comparison**, so `-1 < v.len()`
   is "literal `-1` does not fit in usize", even though comparisons now work
   across types. Rare, but the fix is easy: give such a literal `i64` instead.
5. **Seeing the game's output needed tools outside Om.** To check the 3D view
   I wrote a Python harness that runs the game in a pseudo-terminal, sends keys
   on a timeline and turns the ANSI output into PNG screenshots. Most of the
   playtesting time went into that harness and two bugs in it. A standard
   "render this terminal output to an image" tool (or an Om-side headless
   frame dump) would help every terminal game.
6. **One file per program** (as before): 2,200 lines. No new pain beyond what
   The Hollow Bell's notes say.

## Things that went better than expected

- Float maths compiles to tight native code: 200 frames of raycasting, floor
  casting with per-pixel lighting, sprites and terminal output take 2 ms each
  at 100x32 and 7 ms at 200x60, so the game runs at a steady 30 frames a second
  with most of the frame spent sleeping.
- Plain structs being copied made the entity code simple: read `let e =
  self.ents[i]`, compute, write back the fields that changed.
- `let (w, h, z) = if ... { (...) } else if ... { (...) } else { (...) }` and
  tuple returns read well for per-kind parameters.
- The language changes from the first game's notes paid off at once: the
  first full draft of 1,500 lines compiled after five fixes, against dozens of
  round trips for The Hollow Bell.

## Balance log (autopilot, 40 games per row)

The autopilot only knows what it has seen, reacts after about a third of a
second, flares when a shade is on it or a hound is about to bite, and never
lights braziers.

| Change | Wins | Deaths: shades / hounds |
|---|---|---|
| First version, autopilot knows the whole map | 6 of 20 | 0 / 14 |
| Autopilot explores instead; oil drain 0.95 to 0.7 per second; hounds slower (could outrun you at depth 6) | 12 | 6 / 22 |
| A flare burns a hound within 2 steps; autopilot flares at a hound 2 steps away | 37 | 2 / 1 |
| More oil drain, flares cost 20, one more hound and shade per level | 37 | 2 / 1 |
| Autopilot waits a third of a second before reacting | 35 | 4 / 1 |
| Shades bolder (light must be brighter to hold them off), lunge more often, hit harder; slower healing; fewer oil flasks | 22 | 10 / 8 |

Screenshots found: the scene was too dark to read (the lantern's falloff is now
softer and brighter); crystal walls were noisy (thinner veins); the title
lettering had a broken B; a dev-key test was "failing" because the harness
dropped `--dev` from the command line.
