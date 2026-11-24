# kernel

Pure math over plain values: no ECS, no world, no systems.
It imports nothing else from the crate, and from Bevy only `bevy::math` (glam's `Vec2`, `Rect`).

## Why it exists

The AI must predict the ball with exactly the physics that moves it (spec 1.2).
"Exactly" is guaranteed only if both call one function.
A function both can call depends on neither, so it is plain math.
Plain math is also unit-testable without an `App`.

## Files

| File | Holds | Shape and reason |
|---|---|---|
| `mod.rs` | `Range`, `OutOfRange` | One table of accepted value sets. Each variant owns its test and its wording, so they cannot drift. Tuning reports these instead of keeping its own table. |
| `geometry.rs` | `Side`, `Radius`, `LengthBounds`, `Arena` | Private fields, checked `new` returning `Result`. Holding one proves it valid. |
| `integrate.rs` | `Motion`, `integrate`, `advance_between_walls`, `WallStep` | Semi-implicit Euler: velocity first, then position, stable under the oscillating forces later twists add. The wall step bounds the vertical speed to what its one fold undoes. |
| `collide.rs` | `swept_circle_rect`, `Impact`, `Face` | A swept circle is a ray against the rectangle grown by the radius, solved per axis (slab method). Corners are square: every contact lands on a face. A start that overlaps is a contact at time zero, by any depth and whichever way the ball moves, so a mirror there turns a leaving ball back in (M2 plan 9.1); a start that only touches and moves away is none, so a contact that ends a step on a face is not found again by the next step. Paddles are swept through it bare; obstacles through `obstacle.rs`. |
| `obstacle.rs` | `Obstacle`, `Contact`, `swept_circle_obstacle`, `BounceRule`, `ObstaclesInFlight`, `ObstacleStep`, `advance_among_obstacles` | The obstacle channel (M2 spec 2.2.6). An `Obstacle` is a rectangle at any tilt; its sweep turns the path into the obstacle's own frame and sweeps there, and a start within a skin of a face, leaving or sliding, is no contact, since rounding leaves a bounced ball a hair inside. `advance_among_obstacles` is the step physics and prediction both take: walls and obstacles in one contact loop. A `BounceRule` holds the base's four bounds: contacts per step, the horizontal floor, the speed a ball leaves an obstacle at, and the bounce budget per flight, after which the ball flies through obstacles. With no obstacle, or a spent budget, the step is `advance_between_walls` to the last bit. |
| `reflect.rs` | `HitOffset`, `MaxDeflection`, `paddle_reflection`, `mirror`, `SpeedRule` | Paddles aim by hit offset instead of mirroring, which gives the player control. Walls mirror. `launch_speed` adds the effects' gain to the serve speed and applies the cap: the one statement of the speed channel's bound. A `SpeedRule` is one mode's per-hit increase with that cap. |
| `flight.rs` | `Field`, `Drift`, `Flight`, `FlightStart`, `Hit`, `Contribution`, `StepMotion` | What holds from a serve or return to the next (ADR 0011). A `Field` is a pure lateral force; `Drift` is the one shipped, a steady pull stated as displacement per crossing, in the kernel so that effects sharing it need not import each other; a `Flight` sums the fields and gains contributed at its start, multiplies the paddle height factors, and holds the obstacles contributed, with the rule the base gave it to bounce off them by and the bounces it has made. It keeps the `FlightStart` it began at and the seed of the rally it began in (`rally()`), which the next rally's seed does not replace, and `believed_as` composes the same flight, start, rally, and clock kept, from the contributions an AI believes in (M2 spec 2.3.12). Its clock counts steps, not seconds, so physics and prediction read the same time exactly. |
| `ramp.rs` | `Ramp`, `FullBeforeStart`, `ItemCount`, `InvalidItemCount` | The curve every effect's dial rides: a start value until one return, linear to a full value at another (M2 spec 2.3.9). `new` refuses a full return before the start return. `ItemCount` is its whole-count form, for an effect whose dial is a number of items: the whole part of the dial from none to a full count, reckoned in whole numbers. Its `new` also refuses a count that starts to move at return zero, so no item lies on the field at a serve. |
| `predict.rs` | `crossing_y` | Steps the same `advance_among_obstacles` at the same `dt` under the same `Flight` the physics uses, reading the flight's clock ahead without advancing it and spending its bounce budget ahead. A ball an obstacle turns back for good is followed to the far goal line and given no crossing. |
| `random.rs` | `Pcg32`, `RallySeed` | Hand-rolled: reproducible from one `u64`, with no crate on the dependency treadmill. A `RallySeed` is one rally's number, from which effects derive what they choose once per rally. |
| `test_support.rs` | `assert_near`, `assert_vec_near`, fixtures | Tests only. Exact float equality is a lint error; every assertion states its tolerance. |

## Invariants by construction

A kernel value that exists is valid.
`Arena::new`, `Radius::new`, `LengthBounds::new`, `MaxDeflection::new`, and `SpeedRule::new`
return `Err` naming the refused field,
and their fields are private.
No downstream code re-checks a kernel value.

## Why the collision is swept

At the shipped speed cap of 250 units/s, one step at 60 Hz moves the ball 4.17 units.
A paddle is 2 units thick.
An overlap test at the step's end would let the ball pass through.
The sweep finds the first contact along the step's path.

## A bound the wall step keeps

`advance_between_walls` folds an overshoot back inside once.
That is correct only if one step's vertical travel is at most the playable height.
The step keeps that itself (M2 spec 2.2.2): it cuts the vertical speed to the playable height per `dt`,
so the ball ends inside the walls under any force an effect composes.
The bound is derived from the arena and the step, not tuned, and no tuned dial comes near it: 5,280 units/s at the shipped numbers.

Tuning validation still rejects `speed_cap / FIXED_HZ ≥ playable height` at load,
which is why tuning imports `flow::FIXED_HZ`: a launch is then never cut by the bound.
