# The Hollow Bell

*A stealth roguelike of sound, silence and spoken spells, written in Om.*

```text
 #..."""#"#"""....##......@...!.....        THE HOLLOW BELL
 #....""#"#.."....##.........##...."        Depth 3/6  Turn 592
 #........####################   ...        HP     ======== 20/20
                                            Breath ======-- 9/12
                                            Noise |||||||||  walk
```

Far under the mountain, the monastery drowned. The Hollow Bell still hangs at
the bottom. You are a novice monk with no sword: you fight by **chanting
mantras you compose from syllables**, and **every sound you make travels
through the halls**, where some of what lives down here is blind and hunts
only by ear. Reach depth 6, get past the Bell Keeper, and strike the bell.

## Run it

You need the Om compiler (see the [main README](../../README.md)) on Linux,
macOS or WSL, and a terminal of at least 80x24.

```sh
cargo install --path crates/omc        # once, from the repository root
om run games/hollow-bell/hollow_bell.om
```

Or build a native executable: `om build games/hollow-bell/hollow_bell.om -o hollow-bell`.

On the title screen: **Enter** starts a game, **s** starts a game with a seed
you choose (same seed, same dungeon), **d** lets the monk play itself, and **b**
runs the balance benchmark (100 autopilot games, about 30 seconds).

## How to play

| Key | Does |
|---|---|
| arrows, `hjkl yubn`, number pad | move; walk into a creature to strike it |
| `.` or space | wait and catch your breath |
| `s` | sneak: silent footsteps, but each step takes two turns |
| `x` | explore on your own until something happens |
| `>` | walk to the stairs and go down |
| `c` | chant: type syllables, Enter, then aim (a direction, or Enter for the nearest foe) |
| `r` / `C` | repeat your last mantra / recall one of the last four |
| `q` / `e` | eat a herb / burn incense (reveals the level) |
| `?` / `Q` | help / give up |

### Mantras

A mantra is one **form** (where it goes) followed by one or more **forces**
(what it does). Repeating a force makes it stronger. You start knowing KA, OM
and VU; carved tablets (`?` on the map) teach the rest, and bell shards (`*`)
deepen your breath and let mantras grow longer.

| Form | | Force | |
|---|---|---|---|
| `ka` | bolt, flies until it hits | `om` | strike (4 damage each) |
| `ra` | ring around you | `hai` | flame; fire spreads through grass |
| `mu` | wave, a cone in front of you | `vu` | push; walls hurt, chasms kill |
| `su` | seed, bursts where it lands | `na` | still; holds creatures in place |
| | | `sha` | hush; sound cannot cross that ground |
| | | `lo` | lure; a loud false noise |

Some combinations: `ka om om` (a heavy bolt), `ka vu` (shove a monk into a
chasm), `su lo` (throw your voice to draw a listener away), `ra sha` (a pool of
silence to chant in), `mu hai` (a fan of fire into a grassy room), `ra vu`
(clear everything off you), `su na` (freeze a group).

### Sound

Everything makes noise, and the Noise meter shows how much your last action
made; the floor it reached glows for a moment. Sound flows through open
ground, is muffled by closed doors, and stops dead at hushed (blue) ground.
Chanting is loud unless the mantra contains `sha`. Water splashes. Sleeping
creatures take triple damage from your strikes, unaware ones double. Tall grass
hides you from eyes more than two steps away. Listeners (`L`) are blind and hear
twice as far as anything else. The longer you stay on a level, the more the
mountain groans and wakes things up.

## What is in it

About 2,400 lines of Om, in one file:

- **Level generator**: rooms, L-shaped corridors with extra loops, doors where
  corridors meet rooms, random-walk blobs of grass, water and chasms, checked
  for full reachability by flood fill, with the stairs placed at the farthest
  point.
- **Recursive shadowcasting** field of view with remembered tiles.
- **Sound propagation**: a weighted flood fill per sound (closed doors cost 4
  steps, hushed tiles block), compared against each creature's hearing.
- **Creature AI** with sleeping, wandering, searching, hunting and fleeing
  states, energy-based speeds, line-of-sight checks, a shared distance map to
  the player plus per-creature breadth-first pathing. Shades keep their
  distance and sing; golems shake the floor; wisps set grass alight; the Keeper
  holds its vigil and roars when wounded.
- **The mantra system**: a small grammar parsed from typed text, four area
  shapes (Bresenham bolts, rings, cones by angle, bursts), six forces that
  compose, knockback with collisions and chasm falls.
- **Fire** as a cellular automaton over grass and doors.
- **An autopilot** that explores, assassinates sleepers, slips past listeners
  and fights with mantras, used for the demo mode, auto-explore and the balance
  benchmark.

## Balance

The benchmark plays 100 seeded games with the autopilot. With the current
numbers the autopilot wins about one game in six (it fights far more than a
careful player needs to); most of its deaths come from listeners and choir
shades on depths 4 and 5. The numbers that were tuned to get there are noted in
[NOTES.md](NOTES.md), along with everything about Om that made the game harder
to write than it should have been.
