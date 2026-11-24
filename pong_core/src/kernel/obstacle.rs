//! The obstacle channel (M2 spec 2.2.6): solid rectangles at any angle, the
//! sweep that finds a ball's first contact with one, and the rule a ball
//! leaves one by.

use std::num::{NonZeroU8, NonZeroU16};

use bevy::math::{Rect, Vec2};

use super::collide::swept_circle_rect;
use super::geometry::{Arena, Radius};
use super::integrate::{Motion, advance_between_walls, integrate, within_one_fold};
use super::reflect::{MaxDeflection, mirror};

/// How far inside a face a sweep may start and still be leaving it, in
/// arena units. A contact is found in the obstacle's own frame and carried
/// back to the arena's, and the rounding of that can leave the ball a hair
/// inside the face it has just bounced off. The skin is a thousand times
/// that rounding and a thousandth of the shipped ball's radius.
const SKIN: f32 = 1e-3;

/// A solid rectangle on the floor, at any angle.
///
/// Its own frame has x across it and y along it, so an obstacle with no
/// tilt stands upright, as a paddle does.
///
/// Invariant: `across` is the unit vector of its own x axis in the arena's
/// axes, at `tilt` from the arena's x axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obstacle {
    centre: Vec2,
    half_size: Vec2,
    tilt: f32,
    across: Vec2,
}

impl Obstacle {
    /// An obstacle centred on `centre`, reaching `half_size.x` across and
    /// `half_size.y` along from it, turned `tilt` radians counter-clockwise
    /// from upright.
    ///
    /// Precondition (the caller's): all are finite and both half sizes are
    /// positive, as sizes from validated tuning are.
    #[must_use]
    pub fn new(centre: Vec2, half_size: Vec2, tilt: f32) -> Self {
        let (sin, cos) = tilt.sin_cos();
        Self {
            centre,
            half_size,
            tilt,
            across: Vec2::new(cos, sin),
        }
    }

    /// Its middle, in arena units.
    #[must_use]
    pub const fn centre(self) -> Vec2 {
        self.centre
    }

    /// How far it reaches from its centre: `x` across it, `y` along it.
    #[must_use]
    pub const fn half_size(self) -> Vec2 {
        self.half_size
    }

    /// Its turn from upright, counter-clockwise, in radians.
    #[must_use]
    pub const fn tilt(self) -> f32 {
        self.tilt
    }

    /// This obstacle mirrored across midfield, the line `x = 0`: as far to
    /// the other side, at the same height, tilted the other way.
    #[must_use]
    pub fn mirrored_across_midfield(self) -> Self {
        Self::new(
            Vec2::new(-self.centre.x, self.centre.y),
            self.half_size,
            -self.tilt,
        )
    }

    /// A direction of the arena in this obstacle's own axes.
    fn into_own_axes(self, arena: Vec2) -> Vec2 {
        Vec2::new(arena.dot(self.across), arena.dot(self.along()))
    }

    /// A direction in this obstacle's own axes in the arena's.
    fn out_of_own_axes(self, own: Vec2) -> Vec2 {
        self.across * own.x + self.along() * own.y
    }

    /// The unit vector of its own y axis in the arena's axes.
    fn along(self) -> Vec2 {
        Vec2::new(-self.across.y, self.across.x)
    }
}

/// Where and how a sweep first touches an [`Obstacle`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Fraction of the displacement travelled at contact, in `0.0..=1.0`.
    pub toi: f32,
    /// Unit normal of the face struck, pointing out of the obstacle, in the
    /// arena's axes.
    pub normal: Vec2,
}

/// Sweeps a circle of `radius` at `center` along `displacement` and reports
/// its first contact with `obstacle`, or `None` when the sweep misses.
///
/// The path is turned into the obstacle's own frame and swept there with
/// [`swept_circle_rect`], so corners are square, as a paddle's are.
///
/// A sweep that starts within a skin of a face, on it or a hair inside, and
/// leaves through that face or slides along it, misses: it is the sweep
/// after a bounce, begun where the bounce left the ball.
/// [`swept_circle_rect`] calls that start an impact and turns the ball back
/// in (M2 plan 9.1), which a mirror would repeat every step.
///
/// Postcondition: a contact's normal opposes `displacement`, so a mirror on
/// it turns the circle away from the face.
///
/// Nothing here frees a circle that starts deeper inside than the skin: it
/// is in contact at once. Whoever places obstacles keeps them off the ball.
#[must_use]
pub fn swept_circle_obstacle(
    center: Vec2,
    radius: Radius,
    displacement: Vec2,
    obstacle: &Obstacle,
) -> Option<Contact> {
    let start = obstacle.into_own_axes(center - obstacle.centre);
    let travel = obstacle.into_own_axes(displacement);
    let grown = obstacle.half_size + Vec2::splat(radius.value());
    let inside = start.x.abs() <= grown.x && start.y.abs() <= grown.y;
    if inside
        && (leaves_from_the_skin(start.x, travel.x, grown.x)
            || leaves_from_the_skin(start.y, travel.y, grown.y))
    {
        return None;
    }
    let own = Rect::from_center_half_size(Vec2::ZERO, obstacle.half_size);
    let impact = swept_circle_rect(start, radius, travel, own)?;
    Some(Contact {
        toi: impact.toi,
        normal: obstacle.out_of_own_axes(impact.face.normal()),
    })
}

/// Whether a sweep that starts inside a grown rectangle, at `start` on one
/// of its own axes and moving `travel` along it, starts within the skin of
/// the face `reach` out on that axis and does not move in from it.
fn leaves_from_the_skin(start: f32, travel: f32, reach: f32) -> bool {
    reach - start.abs() <= SKIN && travel * start >= 0.0
}

/// The obstacle channel's bounds (M2 spec 2.2.6), the base's and no
/// effect's: how many contacts a ball may make in one fixed step, how steep
/// and how fast it may leave an obstacle, and how often it may bounce off
/// obstacles in one flight.
///
/// The second is the horizontal floor. A mirror can send a ball straight up
/// the arena, where it reaches no goal; a ball leaving an obstacle flies no
/// steeper than `steepest` from level, so it keeps the cosine of that angle
/// as the horizontal share of its speed.
///
/// The third is the bounce budget (M2 spec 2.2.6.1). The floor alone does
/// not end every flight: a ball can bounce between an obstacle, its
/// mirrored copy, and a wall for good (M2 phase 9 report). A flight that
/// has spent its bounces flies through obstacles, and from its last bounce
/// on it keeps its horizontal direction and the floor's share of its speed,
/// so it reaches a goal line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BounceRule {
    contacts_per_step: NonZeroU8,
    steepest: Option<MaxDeflection>,
    bounces_per_flight: NonZeroU16,
    fastest: f32,
}

/// One contact a step, no floor, no budget, and no speed cap: the rule of a
/// flight nobody gave one. A flight that carries obstacles is given the tuned rule by the
/// base.
impl Default for BounceRule {
    fn default() -> Self {
        Self {
            contacts_per_step: NonZeroU8::MIN,
            steepest: None,
            bounces_per_flight: NonZeroU16::MAX,
            fastest: f32::INFINITY,
        }
    }
}

impl BounceRule {
    /// At most `contacts_per_step` contacts in a step, and no steeper than
    /// `steepest` off an obstacle.
    #[must_use]
    pub const fn new(contacts_per_step: NonZeroU8, steepest: MaxDeflection) -> Self {
        Self {
            contacts_per_step,
            steepest: Some(steepest),
            bounces_per_flight: NonZeroU16::MAX,
            fastest: f32::INFINITY,
        }
    }

    /// This rule with a speed cap: a ball leaves an obstacle no faster than
    /// `speed`. Without it a ball leaves at the speed it came at.
    ///
    /// The cap is the launch's (M2 spec 2.2.2): a ball in a field may fly
    /// faster than it, by its vertical speed only. A bounce turns vertical
    /// speed into horizontal, which the swept paddle contact and the
    /// sanity phase both hold to the cap.
    ///
    /// Precondition (the caller's): `speed` is positive, as a validated
    /// speed cap is.
    #[must_use]
    pub const fn no_faster_than(self, speed: f32) -> Self {
        Self {
            fastest: speed,
            ..self
        }
    }

    /// This rule with a budget: at most `bounces` bounces off obstacles in
    /// one flight. Without it a rule has none.
    #[must_use]
    pub const fn with_bounces_per_flight(self, bounces: NonZeroU16) -> Self {
        Self {
            bounces_per_flight: bounces,
            ..self
        }
    }

    /// The most bounces off obstacles a ball makes in one flight. Once it
    /// has made them it flies through obstacles until its next flight.
    #[must_use]
    pub const fn bounces_per_flight(self) -> NonZeroU16 {
        self.bounces_per_flight
    }

    /// The most contacts, with walls and obstacles together, a ball makes
    /// in one fixed step among obstacles.
    #[must_use]
    pub const fn contacts_per_step(self) -> NonZeroU8 {
        self.contacts_per_step
    }

    /// The velocity of a ball that struck an obstacle's face of unit
    /// `normal` at `incoming`.
    ///
    /// The ball is mirrored on the face. If that leaves it steeper than the
    /// floor allows, it is turned to the floor's angle on the side the
    /// mirror sent it, or on the other side where that one would turn it
    /// back into the face. A ball mirrored straight up or down has no side:
    /// it takes the one the face looks to, and off a level face `+x`.
    ///
    /// Then, with a speed cap, a ball faster than it is slowed to it along
    /// the course it has.
    ///
    /// Precondition (the caller's): `incoming` moves into the face, against
    /// `normal`, as a [`Contact`]'s sweep does.
    /// Postcondition: the speed is `incoming`'s, or the cap where that is
    /// less; the ball does not move into the face; with a floor, its
    /// horizontal share of the speed is at least the cosine of the floor's
    /// angle.
    #[must_use]
    pub fn leaving(self, incoming: Vec2, normal: Vec2) -> Vec2 {
        let leaving = self.floored(mirror(incoming, normal), normal);
        let speed = leaving.length();
        if speed > self.fastest {
            leaving * (self.fastest / speed)
        } else {
            leaving
        }
    }

    /// `mirrored`, the ball as a mirror on the face of `normal` left it,
    /// turned to the floor's angle where it is steeper.
    fn floored(self, mirrored: Vec2, normal: Vec2) -> Vec2 {
        let Some(steepest) = self.steepest else {
            return mirrored;
        };
        let speed = mirrored.length();
        let (sin, cos) = steepest.radians().sin_cos();
        if mirrored.x.abs() >= cos * speed {
            return mirrored;
        }
        let side = if mirrored.x == 0.0 {
            normal.x
        } else {
            mirrored.x
        };
        let turned = Vec2::new(cos.copysign(side), sin.copysign(mirrored.y)) * speed;
        if turned.dot(normal) < 0.0 {
            Vec2::new(-turned.x, turned.y)
        } else {
            turned
        }
    }
}

/// The obstacles of a flight as the ball meets them at one moment of it:
/// what stands on the floor, the rule the ball bounces off it by, and the
/// bounces the flight has made so far.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObstaclesInFlight<'a> {
    /// The obstacles standing on the floor, in no meaningful order.
    pub obstacles: &'a [Obstacle],
    /// How the ball bounces off them, and how often.
    pub rule: BounceRule,
    /// Bounces off obstacles in the flight so far.
    pub bounced: u16,
}

impl ObstaclesInFlight<'_> {
    /// These obstacles once the flight has made `bounces` more bounces.
    #[must_use]
    pub fn after(self, bounces: u8) -> Self {
        Self {
            bounced: self.bounced.saturating_add(u16::from(bounces)),
            ..self
        }
    }

    /// Whether the flight has spent its bounces: the ball then flies
    /// through the obstacles.
    #[must_use]
    pub const fn spent(self) -> bool {
        self.bounced >= self.rule.bounces_per_flight().get()
    }
}

/// Result of [`advance_among_obstacles`]: where the ball is now and what
/// it bounced off on the way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObstacleStep {
    /// Position and velocity after the step.
    pub motion: Motion,
    /// Bounces off the top or bottom wall during the step.
    pub wall_bounces: u8,
    /// Bounces off obstacles during the step.
    pub obstacle_bounces: u8,
}

/// One fixed step of a ball of `radius` inside `arena` among the obstacles
/// of its flight: the step both the physics and the AI's prediction take
/// (M2 spec 2.2.6).
///
/// With no obstacle, or in a flight that has spent its bounces, it is
/// [`advance_between_walls`], to the last bit.
///
/// Among obstacles the velocity is integrated and bounded as there, and the
/// position then moves along it leg by leg: to the first contact with a
/// wall or an obstacle, off it, and on with what is left of the step. A
/// wall mirrors; an obstacle is left by the rule
/// ([`BounceRule::leaving`]). Contacts at the same moment are settled by
/// the contact and not by the order of the obstacles, and a wall goes
/// before an obstacle.
///
/// The loop is bounded: after the rule's contacts in one step the ball
/// flies on to the next contact, if the step holds one, and stops there
/// with the rest of the step forfeited. It bounces off it in the next step.
/// And the flight is bounded: the bounce that spends its budget is made,
/// and from there on the step meets walls only.
///
/// Precondition (the caller's): `motion` is inside the walls, `dt` is not
/// negative, and unless the flight has spent its bounces the ball does not
/// start deeper inside an obstacle than [`swept_circle_obstacle`]'s skin.
/// The caller adds the step's bounces to the flight's
/// ([`ObstaclesInFlight::after`]) before the next step.
/// Postcondition: the ball ends inside the walls, to rounding, and has
/// passed through no obstacle while the flight had a bounce left; it made
/// at most the rule's contacts in the step and its bounces in the flight; a
/// bounce off an obstacle kept its speed, or cut it to the rule's cap.
#[must_use]
pub fn advance_among_obstacles(
    arena: Arena,
    radius: Radius,
    motion: Motion,
    force: Vec2,
    dt: f32,
    in_flight: ObstaclesInFlight,
) -> ObstacleStep {
    if in_flight.obstacles.is_empty() || in_flight.spent() {
        let step = advance_between_walls(arena, radius, motion, force, dt);
        return ObstacleStep {
            motion: step.motion,
            wall_bounces: u8::from(step.bounced),
            obstacle_bounces: 0,
        };
    }
    let rule = in_flight.rule;
    let limit = arena.center_limit_y(radius);
    let bounded = within_one_fold(motion, integrate(motion, force, dt), 2.0 * limit, dt);
    let mut step = ObstacleStep {
        motion: Motion {
            position: motion.position,
            velocity: bounded.velocity,
        },
        wall_bounces: 0,
        obstacle_bounces: 0,
    };
    let mut seconds_left = dt;
    loop {
        let leg = step.motion.velocity * seconds_left;
        let solid: &[Obstacle] = if in_flight.after(step.obstacle_bounces).spent() {
            &[]
        } else {
            in_flight.obstacles
        };
        let Some(first) = first_contact(step.motion.position, radius, leg, limit, solid) else {
            step.motion.position += leg;
            return step;
        };
        step.motion.position += leg * first.toi();
        if let Surface::Wall { y, .. } = first {
            // On the wall exactly, so that rounding never leaves it past it.
            step.motion.position.y = y;
        }
        if step.wall_bounces + step.obstacle_bounces == rule.contacts_per_step().get() {
            return step;
        }
        seconds_left *= 1.0 - first.toi();
        match first {
            Surface::Wall { .. } => {
                step.motion.velocity = mirror(step.motion.velocity, Vec2::Y);
                step.wall_bounces += 1;
            }
            Surface::Obstacle(contact) => {
                step.motion.velocity = rule.leaving(step.motion.velocity, contact.normal);
                step.obstacle_bounces += 1;
            }
        }
    }
}

/// What a leg of a step meets first.
#[derive(Debug, Clone, Copy)]
enum Surface {
    /// The top or bottom wall, where the ball's centre turns at `y`,
    /// reached at `toi` of the leg.
    Wall { toi: f32, y: f32 },
    /// An obstacle.
    Obstacle(Contact),
}

impl Surface {
    /// Fraction of the leg travelled at contact.
    const fn toi(self) -> f32 {
        match self {
            Self::Wall { toi, .. } => toi,
            Self::Obstacle(contact) => contact.toi,
        }
    }
}

/// The first thing a ball of `radius` at `position` meets along `leg`: a
/// wall, where its centre passes `±limit`, or one of `obstacles`. A wall
/// goes before an obstacle met at the same moment, and of two obstacles met
/// at the same moment the one whose normal is least, by x and then by y:
/// an order of the contacts, whatever the order of `obstacles`.
fn first_contact(
    position: Vec2,
    radius: Radius,
    leg: Vec2,
    limit: f32,
    obstacles: &[Obstacle],
) -> Option<Surface> {
    let end_y = position.y + leg.y;
    let wall = if leg.y > 0.0 && end_y > limit {
        Some(limit)
    } else if leg.y < 0.0 && end_y < -limit {
        Some(-limit)
    } else {
        None
    }
    .map(|y| Surface::Wall {
        // A start a rounding past the wall is on it.
        toi: ((y - position.y) / leg.y).max(0.0),
        y,
    });
    let obstacle = obstacles
        .iter()
        .filter_map(|obstacle| swept_circle_obstacle(position, radius, leg, obstacle))
        .min_by(|one, other| {
            one.toi
                .total_cmp(&other.toi)
                .then(one.normal.x.total_cmp(&other.normal.x))
                .then(one.normal.y.total_cmp(&other.normal.y))
        })
        .map(Surface::Obstacle);
    match (wall, obstacle) {
        (Some(wall), Some(obstacle)) if obstacle.toi() < wall.toi() => Some(obstacle),
        (wall, obstacle) => wall.or(obstacle),
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_4;
    use std::num::{NonZeroU8, NonZeroU16};

    use bevy::math::{Rect, Vec2};

    use super::{
        BounceRule, Contact, Obstacle, ObstacleStep, ObstaclesInFlight, SKIN,
        advance_among_obstacles, swept_circle_obstacle,
    };
    use crate::kernel::collide::swept_circle_rect;
    use crate::kernel::integrate::{Motion, advance_between_walls};
    use crate::kernel::reflect::{MaxDeflection, mirror};
    use crate::kernel::test_support::{assert_near, assert_vec_near, standard_arena, unit_radius};

    /// A block the size of the default paddle, two thick and fourteen long.
    const BLOCK: Vec2 = Vec2::new(1.0, 7.0);

    fn block_tilted(tilt: f32) -> Obstacle {
        Obstacle::new(Vec2::ZERO, BLOCK, tilt)
    }

    fn contact(center: Vec2, displacement: Vec2, obstacle: &Obstacle) -> Contact {
        swept_circle_obstacle(center, unit_radius(), displacement, obstacle)
            .unwrap_or_else(|| panic!("no contact from {center} along {displacement}"))
    }

    #[test]
    fn an_upright_obstacle_is_struck_as_its_rectangle_is() {
        let centre = Vec2::new(70.0, 0.0);
        let upright = Obstacle::new(centre, BLOCK, 0.0);
        let rect = Rect::from_center_half_size(centre, BLOCK);
        for (center, displacement) in [
            (Vec2::new(60.0, 0.0), Vec2::new(20.0, 0.0)),
            (Vec2::new(80.0, 3.0), Vec2::new(-20.0, -2.0)),
            (Vec2::new(70.0, 20.0), Vec2::new(0.5, -26.0)),
            (Vec2::new(69.0, -20.0), Vec2::new(1.0, 26.0)),
        ] {
            let impact = swept_circle_rect(center, unit_radius(), displacement, rect)
                .expect("each sweep strikes the rectangle");
            let struck = contact(center, displacement, &upright);
            assert_near(struck.toi, impact.toi, 1e-6);
            assert_vec_near(struck.normal, impact.face.normal(), 1e-6);
        }
        assert!(
            swept_circle_obstacle(
                Vec2::new(60.0, 9.0),
                unit_radius(),
                Vec2::new(20.0, 0.0),
                &upright
            )
            .is_none()
        );
    }

    /// M2 spec 2.2.6: the path is swept in the block's own frame. A face
    /// tilted 45 degrees, grown by the ball's radius, lies two units from
    /// the block's centre along its normal: a level ball from ten units
    /// left meets it after `10 - 2√2` units.
    #[test]
    fn a_tilted_face_is_struck_where_the_ball_meets_it_and_gives_its_own_normal() {
        let struck = contact(
            Vec2::new(-10.0, 0.0),
            Vec2::new(10.0, 0.0),
            &block_tilted(FRAC_PI_4),
        );
        assert_near(struck.toi, 1.0 - 2.0_f32.sqrt() / 5.0, 1e-5);
        assert_vec_near(struck.normal, -Vec2::from_angle(FRAC_PI_4), 1e-6);
    }

    #[test]
    fn a_ball_passing_beside_a_tilted_obstacle_misses() {
        // Tilted 45 degrees the block reaches about 5.7 up; grown, under 8.
        assert!(
            swept_circle_obstacle(
                Vec2::new(-20.0, 8.0),
                unit_radius(),
                Vec2::new(40.0, 0.0),
                &block_tilted(FRAC_PI_4)
            )
            .is_none()
        );
    }

    #[test]
    fn a_contacts_normal_opposes_the_sweep() {
        for tilt in tilts() {
            for (center, displacement) in approaches() {
                let block = block_tilted(tilt);
                if let Some(struck) =
                    swept_circle_obstacle(center, unit_radius(), displacement, &block)
                {
                    assert!(
                        struck.normal.dot(displacement) < 0.0,
                        "tilt {tilt}, from {center} along {displacement}: {struck:?}"
                    );
                    assert_near(struck.normal.length(), 1.0, 1e-5);
                }
            }
        }
    }

    /// Every ten degrees from 80 one way to 80 the other.
    fn tilts() -> impl Iterator<Item = f32> {
        (-8_i8..=8).map(|tens| (f32::from(tens) * 10.0).to_radians())
    }

    /// Sweeps toward the block's middle and past its ends, from every side.
    fn approaches() -> Vec<(Vec2, Vec2)> {
        let mut approaches = Vec::new();
        for degrees in (0_u16..360).step_by(15) {
            let from = Vec2::from_angle(f32::from(degrees).to_radians()) * 30.0;
            for aim in [
                Vec2::ZERO,
                Vec2::new(0.4, 5.0),
                Vec2::new(-0.7, -6.5),
                Vec2::new(2.5, 1.0),
            ] {
                approaches.push((from, (aim - from) * 1.5));
            }
        }
        approaches
    }

    /// M2 plan 9.1: the sweep after a bounce starts where the bounce left
    /// the ball, on the face to rounding, and must not be a second contact.
    #[test]
    fn the_sweep_after_a_bounce_is_no_second_contact_at_any_tilt() {
        let mut bounces = 0_u32;
        for tilt in tilts() {
            let block = block_tilted(tilt);
            for (center, displacement) in approaches() {
                let Some(struck) =
                    swept_circle_obstacle(center, unit_radius(), displacement, &block)
                else {
                    continue;
                };
                bounces += 1;
                let at = center + displacement * struck.toi;
                let away = mirror(displacement, struck.normal);
                assert_eq!(
                    swept_circle_obstacle(at, unit_radius(), away, &block),
                    None,
                    "tilt {tilt}, from {center} along {displacement}"
                );
            }
        }
        assert!(bounces > 1_000, "only {bounces} sweeps struck the block");
    }

    /// The point a hair `inside` the grown face a tilted block turns down
    /// and left, level with the block's centre along the face.
    fn a_hair_inside_the_face(tilt: f32, inside: f32) -> Vec2 {
        -Vec2::from_angle(tilt) * (2.0 - inside)
    }

    #[test]
    fn a_sweep_that_starts_a_hair_inside_a_face_and_leaves_through_it_misses() {
        for tilt in tilts() {
            let out = -Vec2::from_angle(tilt);
            let along = Vec2::new(-out.y, out.x);
            for leaving in [out, out + along * 3.0, out * 0.05 - along * 2.0] {
                assert_eq!(
                    swept_circle_obstacle(
                        a_hair_inside_the_face(tilt, 1e-4),
                        unit_radius(),
                        leaving,
                        &block_tilted(tilt)
                    ),
                    None,
                    "tilt {tilt}, leaving along {leaving}"
                );
            }
        }
    }

    #[test]
    fn a_sweep_that_slides_along_the_face_it_starts_on_misses() {
        for tilt in tilts() {
            let out = -Vec2::from_angle(tilt);
            let along = Vec2::new(-out.y, out.x);
            for inside in [0.0, 1e-4] {
                assert_eq!(
                    swept_circle_obstacle(
                        a_hair_inside_the_face(tilt, inside),
                        unit_radius(),
                        along * 2.0,
                        &block_tilted(tilt)
                    ),
                    None,
                    "tilt {tilt}, {inside} inside"
                );
            }
        }
    }

    /// The other half of the boundary: on the face or a hair inside it and
    /// moving in is a contact now, on that face.
    #[test]
    fn a_sweep_that_starts_a_hair_inside_a_face_and_moves_in_strikes_it_at_once() {
        for tilt in tilts() {
            let out = -Vec2::from_angle(tilt);
            let along = Vec2::new(-out.y, out.x);
            for inside in [0.0, 1e-4] {
                let struck = contact(
                    a_hair_inside_the_face(tilt, inside),
                    along - out,
                    &block_tilted(tilt),
                );
                assert_near(struck.toi, 0.0, 1e-6);
                assert_vec_near(struck.normal, out, 1e-5);
            }
        }
    }

    /// Nothing frees a ball from deep inside: it is in contact at once,
    /// as M2 plan 9.1 found of the rectangle sweep. Placement keeps blocks
    /// off the ball.
    #[test]
    fn a_sweep_that_starts_deeper_inside_than_the_skin_is_in_contact_at_once() {
        let struck = contact(
            a_hair_inside_the_face(FRAC_PI_4, 0.5),
            -Vec2::from_angle(FRAC_PI_4),
            &block_tilted(FRAC_PI_4),
        );
        assert_near(struck.toi, 0.0, 1e-6);
    }

    #[test]
    fn the_mirrored_obstacle_is_struck_as_the_obstacle_is_mirrored() {
        let block = Obstacle::new(Vec2::new(-30.0, 12.0), BLOCK, 0.5);
        let copy = block.mirrored_across_midfield();
        assert_vec_near(copy.centre(), Vec2::new(30.0, 12.0), 1e-6);
        assert_vec_near(copy.half_size(), BLOCK, 1e-6);
        assert_near(copy.tilt(), -0.5, 1e-6);
        let center = Vec2::new(-50.0, 10.0);
        let displacement = Vec2::new(30.0, 3.0);
        let struck = contact(center, displacement, &block);
        let mirrored = contact(
            Vec2::new(-center.x, center.y),
            Vec2::new(-displacement.x, displacement.y),
            &copy,
        );
        assert_near(mirrored.toi, struck.toi, 1e-5);
        assert_vec_near(
            mirrored.normal,
            Vec2::new(-struck.normal.x, struck.normal.y),
            1e-5,
        );
    }

    fn four_contacts() -> NonZeroU8 {
        NonZeroU8::new(4).expect("four is not zero")
    }

    /// No steeper than sixty degrees from level: half the speed stays
    /// horizontal.
    fn floored() -> BounceRule {
        let sixty = MaxDeflection::new(60.0_f32.to_radians()).expect("sixty degrees is valid");
        BounceRule::new(four_contacts(), sixty)
    }

    /// How steep `velocity` flies, from level, in degrees.
    fn steepness(velocity: Vec2) -> f32 {
        velocity.y.abs().atan2(velocity.x.abs()).to_degrees()
    }

    #[test]
    fn a_ball_that_leaves_a_face_level_enough_is_mirrored() {
        // An upright face: a ball flying in at thirty degrees leaves at thirty.
        let incoming = Vec2::from_angle(30.0_f32.to_radians()) * 100.0;
        let leaving = floored().leaving(incoming, Vec2::NEG_X);
        assert_vec_near(leaving, mirror(incoming, Vec2::NEG_X), 1e-4);
    }

    /// M2 spec 2.2.6 and its fact: a level ball on a 45° face leaves
    /// vertical under a mirror. The floor turns it to its steepest angle,
    /// on the side the face looks to, at the speed it came at.
    #[test]
    fn a_level_ball_on_a_45_degree_face_leaves_at_the_floors_angle_and_its_own_speed() {
        let face = Vec2::from_angle(135.0_f32.to_radians());
        let leaving = floored().leaving(Vec2::new(100.0, 0.0), face);
        assert_near(steepness(leaving), 60.0, 1e-3);
        assert_near(leaving.length(), 100.0, 1e-3);
        assert!(leaving.y > 0.0, "{leaving}");
        assert!(leaving.dot(face) > 0.0, "{leaving}");
    }

    #[test]
    fn the_floor_keeps_the_side_the_mirror_sent_the_ball_to() {
        // Off a level face from below-left the mirror sends the ball on to
        // the right at eighty degrees; from below-right, on to the left.
        for side in [1.0, -1.0] {
            let incoming = Vec2::new(side * 80.0_f32.to_radians().cos(), -0.985) * 100.0;
            let leaving = floored().leaving(incoming, Vec2::Y);
            assert_near(steepness(leaving), 60.0, 1e-3);
            assert!(leaving.x * side > 0.0 && leaving.y > 0.0, "{leaving}");
        }
    }

    /// Where the mirror's side would turn the ball back into the face, the
    /// floor takes the other. A face looking twenty degrees up from the
    /// right mirrors this ball to ten degrees left of straight up; turned
    /// to sixty degrees on the left it would fly into the face.
    #[test]
    fn the_floor_takes_the_other_side_where_the_mirrors_would_turn_the_ball_into_the_face() {
        let face = Vec2::from_angle(20.0_f32.to_radians());
        let mirrored = Vec2::from_angle(100.0_f32.to_radians()) * 100.0;
        let leaving = floored().leaving(mirror(mirrored, face), face);
        assert_near(steepness(leaving), 60.0, 1e-3);
        assert!(leaving.x > 0.0 && leaving.y > 0.0, "{leaving}");
        assert!(leaving.dot(face) > 0.0, "{leaving}");
    }

    #[test]
    fn off_any_face_the_ball_keeps_its_speed_the_floors_share_and_leaves_the_face() {
        let floor = 60.0_f32.to_radians().cos();
        for face_degrees in (0_i16..360).step_by(5) {
            let face = Vec2::from_angle(f32::from(face_degrees).to_radians());
            for into in (-85_i16..=85).step_by(5) {
                let incoming =
                    -Vec2::from_angle(f32::from(face_degrees + into).to_radians()) * 120.0;
                let leaving = floored().leaving(incoming, face);
                assert_near(leaving.length(), 120.0, 1e-2);
                assert!(
                    leaving.x.abs() >= floor * 120.0 - 1e-2,
                    "face {face_degrees}, into {into}: {leaving}"
                );
                assert!(
                    leaving.dot(face) >= -1e-2,
                    "face {face_degrees}, into {into}: {leaving}"
                );
            }
        }
    }

    /// M2 spec 2.2.2: the speed cap is the launch's, and a ball in a field
    /// may fly faster than it by its vertical speed only. A bounce turns
    /// vertical speed into horizontal, so a ball leaves an obstacle no
    /// faster than the cap; one that came slower leaves at its own speed.
    #[test]
    fn a_ball_leaves_an_obstacle_no_faster_than_the_speed_cap() {
        let capped = floored().no_faster_than(250.0);
        // 250 across and 190 down, as a field leaves a ball: 314 a second.
        let fast = Vec2::new(250.0, -190.0);
        for face in [Vec2::NEG_X, Vec2::Y, Vec2::from_angle(2.5)] {
            if fast.dot(face) >= 0.0 {
                continue;
            }
            let leaving = capped.leaving(fast, face);
            assert_near(leaving.length(), 250.0, 1e-2);
            assert!(leaving.dot(face) > 0.0, "{leaving}");
            let uncapped = floored().leaving(fast, face);
            assert_near(uncapped.length(), fast.length(), 1e-2);
            // Slowed along its own course, not turned.
            assert_vec_near(leaving, uncapped * (250.0 / fast.length()), 1e-2);
        }
        let slow = capped.leaving(Vec2::new(80.0, 0.0), Vec2::NEG_X);
        assert_vec_near(slow, Vec2::new(-80.0, 0.0), 1e-4);
    }

    #[test]
    fn a_rule_with_no_floor_mirrors_the_ball_however_steep() {
        let face = Vec2::from_angle(135.0_f32.to_radians());
        let leaving = BounceRule::default().leaving(Vec2::new(100.0, 0.0), face);
        assert_vec_near(leaving, Vec2::new(0.0, 100.0), 1e-3);
        assert_eq!(BounceRule::default().contacts_per_step().get(), 1);
        assert_eq!(floored().contacts_per_step().get(), 4);
    }

    const DT: f32 = 1.0 / 60.0;
    /// The ball's centre bounces at ±44.
    const LIMIT: f32 = 44.0;

    fn step_among(
        motion: Motion,
        force: Vec2,
        obstacles: &[Obstacle],
        rule: BounceRule,
    ) -> ObstacleStep {
        step_after(0, motion, force, obstacles, rule)
    }

    /// A step of a flight that has bounced off obstacles `bounced` times.
    fn step_after(
        bounced: u16,
        motion: Motion,
        force: Vec2,
        obstacles: &[Obstacle],
        rule: BounceRule,
    ) -> ObstacleStep {
        advance_among_obstacles(
            standard_arena(),
            unit_radius(),
            motion,
            force,
            DT,
            ObstaclesInFlight {
                obstacles,
                rule,
                bounced,
            },
        )
    }

    /// [`floored`], with at most `bounces` bounces off obstacles in a flight.
    fn budgeted(bounces: u16) -> BounceRule {
        floored().with_bounces_per_flight(NonZeroU16::new(bounces).expect("a budget is not zero"))
    }

    /// M2 spec 2.2.6.1, taken by the user's ruling (M2 phase 9 report): a
    /// flight has a budget of bounces off obstacles. Once it is spent the
    /// ball flies through them as through air, to the last bit, until its
    /// next flight.
    #[test]
    fn a_flight_that_has_spent_its_bounces_flies_through_obstacles_as_through_air() {
        let block = Obstacle::new(Vec2::new(10.0, 0.0), BLOCK, 0.0);
        let motion = flying(Vec2::new(7.0, 0.0), Vec2::new(120.0, 30.0));
        let force = Vec2::new(0.0, 40.0);
        let walls = advance_between_walls(standard_arena(), unit_radius(), motion, force, DT);
        for bounced in [6, 7, u16::MAX] {
            let step = step_after(bounced, motion, force, &[block], budgeted(6));
            assert_eq!(step.motion, walls.motion);
            assert_eq!(step.obstacle_bounces, 0);
        }
        let solid = step_after(5, motion, force, &[block], budgeted(6));
        assert_eq!(solid.obstacle_bounces, 1);
        assert!(solid.motion.velocity.x < 0.0, "{solid:?}");
    }

    /// The budget's last bounce is made, and what is left of that step
    /// flies through. Two upright blocks face each other four units apart,
    /// to a ball's centre: at 480 a second one step, eight units long,
    /// bounces off both.
    #[test]
    fn the_last_bounce_of_the_budget_is_made_and_the_rest_of_the_step_flies_through() {
        let pair = [
            Obstacle::new(Vec2::new(4.0, 0.0), BLOCK, 0.0),
            Obstacle::new(Vec2::new(-4.0, 0.0), BLOCK, 0.0),
        ];
        let motion = flying(Vec2::new(0.0, 0.0), Vec2::new(480.0, 0.0));
        let both = step_after(0, motion, Vec2::ZERO, &pair, budgeted(6));
        assert_eq!(both.obstacle_bounces, 2);
        let last = step_after(5, motion, Vec2::ZERO, &pair, budgeted(6));
        assert_eq!(last.obstacle_bounces, 1);
        // Two units to the right block's face, and six back: through the
        // left block's face at -2, to its middle.
        assert_vec_near(last.motion.position, Vec2::new(-4.0, 0.0), 1e-3);
        assert_vec_near(last.motion.velocity, Vec2::new(-480.0, 0.0), 1e-3);
    }

    #[test]
    fn a_rule_carries_its_budget_and_has_none_until_it_is_given_one() {
        assert_eq!(BounceRule::default().bounces_per_flight().get(), u16::MAX);
        assert_eq!(floored().bounces_per_flight().get(), u16::MAX);
        assert_eq!(budgeted(6).bounces_per_flight().get(), 6);
        assert_eq!(budgeted(6).contacts_per_step().get(), 4);
    }

    fn flying(position: Vec2, velocity: Vec2) -> Motion {
        Motion { position, velocity }
    }

    /// How far outside `obstacle`, grown by the ball's radius, a ball's
    /// centre at `position` is: negative when it is inside.
    fn clear_of(obstacle: &Obstacle, position: Vec2) -> f32 {
        let own = obstacle.into_own_axes(position - obstacle.centre()).abs();
        let outside = own - (obstacle.half_size() + Vec2::splat(1.0));
        outside.x.max(outside.y)
    }

    /// Every earlier test flies with no obstacle, and must not move by a bit.
    #[test]
    fn with_no_obstacle_the_step_is_the_wall_step_to_the_last_bit() {
        for (position, velocity, force) in [
            (Vec2::new(0.0, 0.0), Vec2::new(80.0, 13.7), Vec2::ZERO),
            (
                Vec2::new(-3.1, 43.2),
                Vec2::new(-250.0, 177.3),
                Vec2::new(0.0, 31.0),
            ),
            (
                Vec2::new(12.9, -43.9),
                Vec2::new(61.3, -90.0),
                Vec2::new(0.0, -400.0),
            ),
            (
                Vec2::new(0.0, 40.0),
                Vec2::new(60.0, 0.0),
                Vec2::new(0.0, 1.0e6),
            ),
        ] {
            let motion = flying(position, velocity);
            let walls = advance_between_walls(standard_arena(), unit_radius(), motion, force, DT);
            let step = step_among(motion, force, &[], floored());
            assert_eq!(step.motion, walls.motion);
            assert_eq!(step.wall_bounces, u8::from(walls.bounced));
            assert_eq!(step.obstacle_bounces, 0);
        }
    }

    /// The grown face of an upright block at x = 10 is at x = 8. From 7 at
    /// 120 a second a step travels two units: one to the face, one back.
    #[test]
    fn a_ball_bounces_off_a_block_within_the_step_and_travels_the_rest_of_it() {
        let block = Obstacle::new(Vec2::new(10.0, 0.0), BLOCK, 0.0);
        let step = step_among(
            flying(Vec2::new(7.0, 0.0), Vec2::new(120.0, 0.0)),
            Vec2::ZERO,
            &[block],
            floored(),
        );
        assert_vec_near(step.motion.position, Vec2::new(7.0, 0.0), 1e-4);
        assert_vec_near(step.motion.velocity, Vec2::new(-120.0, 0.0), 1e-4);
        assert_eq!((step.obstacle_bounces, step.wall_bounces), (1, 0));
    }

    #[test]
    fn a_ball_that_meets_no_obstacle_flies_among_them_as_between_the_walls() {
        let block = Obstacle::new(Vec2::new(40.0, 0.0), BLOCK, 0.3);
        for (position, velocity, force) in [
            (
                Vec2::new(0.0, 0.0),
                Vec2::new(80.0, 13.7),
                Vec2::new(0.0, 50.0),
            ),
            (
                Vec2::new(-3.1, 43.2),
                Vec2::new(-250.0, 177.3),
                Vec2::new(0.0, 31.0),
            ),
            (
                Vec2::new(12.9, -43.9),
                Vec2::new(61.3, -90.0),
                Vec2::new(0.0, -400.0),
            ),
        ] {
            let motion = flying(position, velocity);
            let walls = advance_between_walls(standard_arena(), unit_radius(), motion, force, DT);
            let step = step_among(motion, force, &[block], floored());
            assert_vec_near(step.motion.position, walls.motion.position, 1e-4);
            assert_vec_near(step.motion.velocity, walls.motion.velocity, 1e-4);
            assert_eq!(step.wall_bounces, u8::from(walls.bounced));
            assert_eq!(step.obstacle_bounces, 0);
        }
    }

    /// M2 spec 2.2.6, first done-condition: a ball aimed level at a 45°
    /// face still reaches a paddle line. Mirrored, it would fly straight up
    /// and down between the walls for ever.
    #[test]
    fn a_ball_aimed_level_at_a_45_degree_face_still_reaches_a_paddle_line() {
        let block = block_tilted(-FRAC_PI_4);
        let mut motion = flying(Vec2::new(-30.0, 0.0), Vec2::new(80.0, 0.0));
        let mut bounced = false;
        for _ in 0..600 {
            let step = step_among(motion, Vec2::ZERO, &[block], floored());
            bounced |= step.obstacle_bounces > 0;
            motion = step.motion;
            if motion.position.x.abs() >= 73.0 {
                assert!(bounced, "the ball never met the block");
                assert_near(motion.velocity.length(), 80.0, 1e-2);
                return;
            }
        }
        panic!("ten seconds on, the ball is at {motion:?}");
    }

    /// Two blocks leaning toward each other: a channel open at the bottom
    /// that narrows upward and is closed, to a ball, above y = 6.4.
    fn wedge() -> [Obstacle; 2] {
        let lean = 12.0_f32.to_radians();
        [
            Obstacle::new(Vec2::new(-3.4, 0.0), BLOCK, -lean),
            Obstacle::new(Vec2::new(3.4, 0.0), BLOCK, lean),
        ]
    }

    /// M2 spec 2.2.6, second done-condition: a ball in a wedge ends the
    /// step inside the contact bound. Driven up into the channel at six
    /// times the speed cap, so that one step would hold four contacts, it
    /// makes no more in any step than the rule allows and as many in some
    /// step, is never inside a block, and comes back out.
    #[test]
    fn a_ball_in_a_wedge_ends_the_step_inside_the_contact_bound() {
        for bound in [1, 2, 4] {
            let rule = BounceRule::new(
                NonZeroU8::new(bound).expect("no bound here is zero"),
                MaxDeflection::new(60.0_f32.to_radians()).expect("sixty degrees is valid"),
            );
            let blocks = wedge();
            let mut motion = flying(Vec2::new(-1.0, -12.0), Vec2::new(750.0, 1299.0));
            let mut most = 0;
            let mut out = false;
            for _ in 0..600 {
                let step = step_among(motion, Vec2::ZERO, &blocks, rule);
                motion = step.motion;
                let contacts = step.obstacle_bounces + step.wall_bounces;
                assert!(
                    contacts <= bound,
                    "{contacts} contacts under a bound of {bound}"
                );
                most = most.max(contacts);
                for block in &blocks {
                    assert!(
                        clear_of(block, motion.position) > -SKIN,
                        "bound {bound}: inside a block at {}",
                        motion.position
                    );
                }
                if motion.position.x.abs() > 60.0 {
                    out = true;
                    break;
                }
            }
            assert_eq!(most, bound, "no step reached the bound of {bound}");
            assert!(
                out,
                "bound {bound}: the ball is still in the wedge at {motion:?}"
            );
        }
    }

    /// A block's end near the top wall: one step bounces off the block and
    /// then off the wall.
    #[test]
    fn a_wall_and_a_block_in_one_step_are_both_bounced_off() {
        let block = Obstacle::new(Vec2::new(10.0, 40.0), BLOCK, 0.0);
        let step = step_among(
            flying(Vec2::new(7.0, 42.5), Vec2::new(120.0, 120.0)),
            Vec2::ZERO,
            &[block],
            floored(),
        );
        assert_eq!((step.obstacle_bounces, step.wall_bounces), (1, 1));
        assert_vec_near(step.motion.velocity, Vec2::new(-120.0, -120.0), 1e-3);
        assert_vec_near(step.motion.position, Vec2::new(7.0, 43.5), 1e-3);
    }

    #[test]
    fn among_obstacles_the_ball_ends_every_step_inside_the_walls() {
        let block = Obstacle::new(Vec2::new(0.0, 30.0), BLOCK, 1.2);
        let mut motion = flying(Vec2::new(-20.0, 38.0), Vec2::new(150.0, 200.0));
        for _ in 0..300 {
            motion = step_among(motion, Vec2::new(0.0, 900.0), &[block], floored()).motion;
            assert!(motion.position.y.abs() <= LIMIT + 1e-3, "{motion:?}");
        }
    }

    /// M2 spec 2.2: every channel composes commutatively. Two blocks that
    /// cross, struck where their faces meet, are struck at the same moment:
    /// which is bounced off does not depend on the order they are given in.
    #[test]
    fn obstacles_given_in_either_order_give_the_same_step() {
        let one = block_tilted(0.5);
        let other = block_tilted(-0.5);
        let motion = flying(Vec2::new(-4.0, 0.0), Vec2::new(250.0, 0.0));
        let step = step_among(motion, Vec2::ZERO, &[one, other], floored());
        assert_eq!(step.obstacle_bounces, 1);
        assert_eq!(
            step,
            step_among(motion, Vec2::ZERO, &[other, one], floored())
        );
    }
}
