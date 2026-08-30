
### Variables

```stas
let n = 3            ; mutable
const ROLL = "space" ; immutable
n = n + 1
```

Values are integers, floats or strings. Integer arithmetic stays integer;
`+` concatenates when either side is a string.

Variables are referenced as `name` inside expressions and as `$name` inside action
arguments (where a bare word like `down` or `w` is a literal).

```stas
key down $ROLL       ; $ROLL -> "space"
key down space       ; bare word literal
mouse move 100 * 2 0 ; expressions are allowed too
```

### Imports and standard library

Import another STAS file at the top level:

```stas
import "helpers.stas"       ; relative to the importing file
import "lib/combat.stas"    ; relative paths may contain folders
import "std/gamepad"        ; bundled standard library
```
The bundled `std/gamepad` module provides `left_stick(angle, amount)`, `right_stick(angle, amount)`,
all eight `*_stick_<direction>` helpers, and `*_stick_direction(name, amount)`.
Angles are clockwise from up: `up = 0`, `right = 90`, `down = 180`, and `left = 270`.

Three more bundled modules build on top of it and on each other - imports are deduplicated, so
importing several of them (or importing one that imports another) is safe and won't cause
"duplicate function" errors:

- `std/buttons` - game-agnostic press/release/tap helpers: `key_down/up`, `tap_key`,
  `press/release_button`, `tap_button`, `chord_button` (two buttons at once), `set/press/release_trigger`
  and `tap_trigger` (analog triggers via `gamepad axis`), and `mash_button`/`mash_key` for repeated taps.
- `std/movement` - `stick_neutral(side)`, `left_stick_for/hold` and `right_stick_for/hold` (hold a
  stick for N frames, `_hold` variants recenter afterwards), `roll(angle, button, hold)` and
  `roll_direction(direction, button, hold)`, `camera_pan(angle, amount, frames)`, and
  `mouse_look(dx, dy, frames)`.
- `std/ds1` - Dark Souls: Prepare to Die Edition button names and macros for the game's default
  Xbox 360 controller layout (`BTN_ROLL`, `BTN_ATTACK_R`, `AXIS_GUARD_L`, ...; check these against
  your own control settings if you've rebound anything). Macros: `roll_ds1`, `roll_ds1_direction`,
  `attack_right`, `attack_right_heavy`, `attack_left`, `guard(frames)` (hold block / attempt a
  parry), `use_item`, `interact`, `twohand`, `lockon`, `open_menu`, `tas_sprint(angle, cycles)`
  (the stick-wiggle + roll-hold + l1-tap "TAS-sprint" tech - do your own `await focus`/settling
  `wait` before calling it, it starts moving immediately).

See `examples/ds1_macros.stas` for a script that uses all three together, or just
`import "std/all"` to pull in `std/gamepad`, `std/buttons`, `std/movement` and `std/ds1`
at once.

### Functions

```stas
fn tap(k, hold) {
    key down $k
    wait $hold
    key up $k
}

tap("space", 2)
```


### Control flow

```stas
repeat 4 { ... }
while n > 0 { ... }
if n > 2 { ... } else if n == 2 { ... } else { ... }
print n            ; debug output on stderr, not in the .tas file
```

Comparison and logical operators: `== != < <= > >= && || !`.
Arithmetic: `+ - * / %`


### Actions



```
nothing
fps <fps>
frame <frame>
key <down|up> <key>
key_alternative <down|up> <key>
gamepad button <down|up> <button>
gamepad stick <left|right> <angle> <amount 0-1>
gamepad axis <axis> <amount>
mouse button <down|up> <button>
mouse scroll <down|up> <amount>
mouse move <x> <y>
await <focus|ingame|no_ingame|cutscene|no_cutscene|mainmenu|no_mainmenu>
await position <x> <y> <z> <range>
await position_alternative <x> <y> <z> <range>
pause ms <ms>
pause input
```

## Example

```stas
const FORWARD = "w"

fn roll(recovery) {
    key down $FORWARD
    key down space
    wait 2
    key up space
    wait $recovery - 2
    key up $FORWARD
}

await focus
wait 20
repeat 2 {
    roll(24)
    wait 4
}
```

compiles to

```
0 await focus
+20 key down w
+0 key down space
+2 key up space
+22 key up w
+4 key down w
+0 key down space
+2 key up space
+22 key up w
```
