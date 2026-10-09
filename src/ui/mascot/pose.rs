use std::time::Duration;

use crate::app::mascot::Mood;

/// How often the mascot blinks, and for how long.
const BLINK_EVERY: Duration = Duration::from_millis(3_700);
const BLINK: Duration = Duration::from_millis(170);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Eyes {
    Open,
    Closed,
    Happy,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Pose {
    /// How far above its lowest point it floats, in pixels.
    pub(super) lift: usize,
    /// How far right it shakes, from -1 to 1 pixels.
    pub(super) shake: isize,
    /// How far right it looks.
    pub(super) look: usize,
    pub(super) eyes: Eyes,
    pub(super) fringe: usize,
    pub(super) open_mouth: bool,
}

impl Pose {
    pub(super) fn of(mood: Mood, age: Duration) -> Self {
        let millis = age.as_millis();
        let step = |period: u128, steps: u128| (millis % period) * steps / period;
        let blinking = millis % BLINK_EVERY.as_millis() < BLINK.as_millis();
        let open = if blinking { Eyes::Closed } else { Eyes::Open };
        let calm = Self {
            lift: 1,
            shake: 0,
            look: 0,
            eyes: open,
            fringe: 0,
            open_mouth: false,
        };
        match mood {
            Mood::Idle => Self {
                lift: usize::from(step(1_800, 2) == 0),
                fringe: usize::try_from(step(900, 2)).unwrap_or(0),
                ..calm
            },
            Mood::Typing => Self {
                lift: usize::from(step(700, 2) == 0),
                look: 1,
                fringe: usize::try_from(step(400, 2)).unwrap_or(0),
                ..calm
            },
            Mood::Oops => Self {
                lift: 2,
                shake: if step(160, 2) == 0 { -1 } else { 1 },
                eyes: Eyes::Wide,
                open_mouth: true,
                ..calm
            },
            Mood::Happy | Mood::Proud => {
                let hop = [0, 1, 2, 1];
                let period = if mood == Mood::Proud { 480 } else { 720 };
                Self {
                    lift: hop[usize::try_from(step(period, 4)).unwrap_or(0)],
                    eyes: Eyes::Happy,
                    fringe: usize::try_from(step(period / 2, 2)).unwrap_or(0),
                    open_mouth: true,
                    ..calm
                }
            }
            Mood::Asleep => Self {
                lift: 0,
                eyes: Eyes::Closed,
                ..calm
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ghost_floats_while_awake_and_lies_still_asleep() {
        let poses = |mood| -> Vec<Pose> {
            (0..40)
                .map(|step| Pose::of(mood, Duration::from_millis(step * 100)))
                .collect()
        };
        let idle = poses(Mood::Idle);
        assert!(idle.iter().any(|pose| pose.lift != idle[0].lift), "it bobs");
        let asleep = poses(Mood::Asleep);
        assert!(asleep.iter().all(|pose| *pose == asleep[0]), "it rests");
        assert_eq!(asleep[0].eyes, Eyes::Closed);
    }

    #[test]
    fn the_ghost_blinks_now_and_then() {
        assert_eq!(Pose::of(Mood::Idle, Duration::ZERO).eyes, Eyes::Closed);
        assert_eq!(Pose::of(Mood::Idle, BLINK).eyes, Eyes::Open);
        assert_eq!(Pose::of(Mood::Idle, BLINK_EVERY).eyes, Eyes::Closed);
    }
}
