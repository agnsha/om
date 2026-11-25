# Games written in Om

> **In review.** Both games, and the compiler changes they needed, are in
> [pull request #3](https://github.com/agnsha/om/pull/3). They are not on the
> main branch yet, so the commands below work once that change is merged (or on
> its branch).

Two full games were written in Om to find out what the language is like for a
real program of a few thousand lines. Each is a single `.om` file that compiles
to a native executable and runs in your terminal.

## Emberdeep

*A first-person descent by lantern light, in true-colour terminal graphics.*

![The Emberdeep title screen](img/emberdeep-title.png)

You go down into the mountain with a lantern and nothing else. On each of six
levels, three embers glow somewhere in the dark; carry all three to the sealed
gate and it opens onto the stairs down. Your lantern burns oil the whole time.
Shades drift through the rock toward you and drain your life; light burns them,
so they wait at its edge, and as your oil runs low the edge comes closer.
Deeper down, hounds hunt you by smell.

| | |
|---|---|
| ![Ember light on brick](img/emberdeep-ember.png) | ![The gate, open](img/emberdeep-gate.png) |
| ![A shade, with the lantern out](img/emberdeep-shade.png) | ![A hound in the wet caves](img/emberdeep-hound.png) |

### Run it

You need a terminal with true colour and at least 80x24: any modern terminal on
Linux or macOS, or **Windows Terminal** on Windows. A bigger window gives a
sharper picture.

```sh
om run games/emberdeep/emberdeep.om            # a new level layout every time
om run games/emberdeep/emberdeep.om 1234       # the same levels for seed 1234
om build games/emberdeep/emberdeep.om -o emberdeep
```

### Keys

| Key | Does |
|---|---|
| `W` `S` or Up Down | walk forward and back |
| `A` `D` | step sideways |
| Left Right or `Q` `E` | turn |
| Space | flare (20 oil): burns shades and drives hounds off |
| `F` | light a brazier you are standing by (12 oil) |
| `M` or Tab | the map of what you have seen |
| `?` / Esc | help / pause |

On Linux and macOS a terminal only reports key presses, so a tap moves you about
one step. On Windows the game reads the keyboard directly, so movement is smooth.

Extra flags: `--sim 100` plays 100 games with the autopilot to check balance,
`--bench` times 200 frames, and `--dev` turns on test keys.

### What is in it

About 2,200 lines of Om: a raycaster with textured walls, floor and ceiling
drawn as half-block "pixels"; procedural textures from value noise; baked
lighting with shadows plus a per-pixel lantern; billboard sprites; generated
levels; shade and hound AI; and a terminal renderer that only redraws changed
cells (about 2 ms a frame at 100x32).

## The Hollow Bell

*A stealth roguelike of sound, silence and spoken spells.*

```text
 #..."""#"#"""....##......@...!.....        THE HOLLOW BELL
 #....""#"#.."....##.........##...."        Depth 3/6  Turn 592
 #........####################   ...        HP     ======== 20/20
                                            Breath ======-- 9/12
                                            Noise |||||||||  walk
```

Far under the mountain, the monastery drowned. You are a novice monk with no
sword: you fight by **chanting mantras you compose from syllables**, and
**every sound you make travels through the halls**, where some of what lives
down here hunts only by ear. Reach depth 6, get past the Bell Keeper, and
strike the bell.

### Run it

```sh
om run games/hollow-bell/hollow_bell.om
om build games/hollow-bell/hollow_bell.om -o hollow-bell
```

On the title screen: **Enter** starts a game, **s** starts one with a seed you
choose, **d** lets the monk play itself, and **b** runs the balance benchmark.

### Keys

| Key | Does |
|---|---|
| arrows, `hjkl yubn`, number pad | move; walk into a creature to strike it |
| `.` or space | wait and catch your breath |
| `s` | sneak: silent footsteps, but each step takes two turns |
| `x` | explore on your own until something happens |
| `>` | walk to the stairs and go down |
| `c` | chant: type syllables, Enter, then aim |
| `r` / `C` | repeat your last mantra / recall one of the last four |
| `q` / `e` | eat a herb / burn incense (reveals the level) |
| `?` / `Q` | help / give up |

### Mantras

A mantra is one **form** (where it goes) followed by one or more **forces**
(what it does). Repeating a force makes it stronger. You start knowing KA, OM
and VU; carved tablets teach the rest.

| Form | | Force | |
|---|---|---|---|
| `ka` | bolt, flies until it hits | `om` | strike (4 damage each) |
| `ra` | ring around you | `hai` | flame; fire spreads through grass |
| `mu` | wave, a cone in front of you | `vu` | push; walls hurt, chasms kill |
| `su` | seed, bursts where it lands | `na` | still; holds creatures in place |
| | | `sha` | hush; sound cannot cross that ground |
| | | `lo` | lure; a loud false noise |

Try `ka om om` (a heavy bolt), `su lo` (throw your voice to draw a listener
away), `ra sha` (a pool of silence to chant in) or `mu hai` (a fan of fire into
a grassy room).

### What is in it

About 2,400 lines of Om: a level generator checked by flood fill, recursive
shadowcasting for field of view, sound that spreads by weighted flood fill,
creature AI with sleeping, hunting and fleeing states, a small grammar for
mantras, fire as a cellular automaton, and an autopilot used for the demo mode
and balance tests.

## What the games added to Om

Writing the games turned up gaps in the language, and pull request #3 fixes
them: `\u{...}` string escapes, struct fields with default values, `==` on
tuples, structs and simple enums without writing `Eq`, constants as `match`
patterns, and terminal built-ins such as `read_key()`, `poll_key()`,
`key_down(k)`, `term_cols()`, `term_rows()`, `flush()`, `time_ns()` and
`sleep_ms()`.
