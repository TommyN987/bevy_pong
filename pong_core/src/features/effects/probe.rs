//! Test probes: effects no tuning builds, for the tests of the effects base
//! and of the features that read it. Compiled for tests only.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::Effect;
use super::Flaw;
use crate::domain::{Difficulty, EffectId};
use crate::kernel::flight::{Contribution, Field, FlightStart};
use crate::kernel::integrate::Motion;
use crate::kernel::obstacle::Obstacle;

/// A constant push.
#[derive(Debug)]
pub(crate) struct Steady(pub(crate) f32);

impl Field for Steady {
    fn lateral(&self, _motion: Motion, _seconds_in_flight: f32) -> f32 {
        self.0
    }
}

/// Every flaw question an effect was asked.
pub(crate) type FlawLog = Arc<Mutex<Vec<(FlightStart, Difficulty)>>>;

/// An effect an AI misjudges, under Speed's borrowed name: it pushes
/// the ball steadily, and an AI of any difficulty aims and reacts worse
/// by the amounts given and believes the push given.
pub(crate) struct Misjudged {
    push: f32,
    believed_push: Option<f32>,
    added_error: f32,
    added_delay: Duration,
    /// Every flaw question asked so far.
    pub(crate) asked: FlawLog,
}

impl Misjudged {
    /// Pushes by `push`, which the AI believes to be `believed_push`.
    pub(crate) fn pushing(push: f32, believed_push: Option<f32>) -> Self {
        Self {
            push,
            believed_push,
            added_error: 0.0,
            added_delay: Duration::ZERO,
            asked: FlawLog::default(),
        }
    }

    /// Pushes nothing; costs the AI `added_error` and `added_delay`.
    pub(crate) fn costing(added_error: f32, added_delay: Duration) -> Self {
        Self {
            added_error,
            added_delay,
            ..Self::pushing(0.0, Some(0.0))
        }
    }
}

impl Effect for Misjudged {
    fn id(&self) -> EffectId {
        EffectId::Speed
    }

    fn contribute(&self, _start: &FlightStart) -> Contribution {
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: Some(Box::new(Steady(self.push))),
            obstacles: Vec::new(),
        }
    }

    fn flaw(&self, start: &FlightStart, difficulty: Difficulty) -> Flaw {
        self.asked
            .lock()
            .expect("the probe's log is not poisoned")
            .push((*start, difficulty));
        Flaw {
            added_error: self.added_error,
            added_delay: self.added_delay,
            believed: Contribution {
                speed_gain: 0.0,
                paddle_height_factor: 1.0,
                field: self
                    .believed_push
                    .map(|push| Box::new(Steady(push)) as Box<dyn Field>),
                obstacles: Vec::new(),
            },
        }
    }
}

/// An effect that stands obstacles on the floor, under Speed's borrowed
/// name: the same ones in every flight, believed by an AI of any difficulty
/// to be the ones given.
pub(crate) struct Standing {
    /// The obstacles on the floor.
    pub(crate) obstacles: Vec<Obstacle>,
    /// The obstacles an AI believes are there.
    pub(crate) believed: Vec<Obstacle>,
}

impl Standing {
    /// Stands `obstacles` on the floor, seen by every AI as they are.
    pub(crate) fn seen(obstacles: Vec<Obstacle>) -> Self {
        Self {
            believed: obstacles.clone(),
            obstacles,
        }
    }

    fn standing(obstacles: &[Obstacle]) -> Contribution {
        Contribution {
            speed_gain: 0.0,
            paddle_height_factor: 1.0,
            field: None,
            obstacles: obstacles.to_vec(),
        }
    }
}

impl Effect for Standing {
    fn id(&self) -> EffectId {
        EffectId::Speed
    }

    fn contribute(&self, _start: &FlightStart) -> Contribution {
        Self::standing(&self.obstacles)
    }

    fn flaw(&self, _start: &FlightStart, _difficulty: Difficulty) -> Flaw {
        Flaw {
            added_error: 0.0,
            added_delay: Duration::ZERO,
            believed: Self::standing(&self.believed),
        }
    }
}
