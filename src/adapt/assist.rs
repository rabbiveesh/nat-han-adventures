//! The assist dial (0..1) and the concrete levers it pulls.
//!
//! Assists are separate from difficulty: a band says how hard the room *is*, the dial says how
//! much quiet help the physics and level dressing give. Both free play and story mode map the
//! dial to levers with [`AssistLevers::from_dial`], so the game reads one struct either way.
//! None of this is ever named on screen.

/// Concrete, silent help derived from the assist dial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssistLevers {
    /// Multiply [`crate::game::tuning::COYOTE_TIME`] by this (1.0 ..= 2.0).
    pub coyote_mult: f32,
    /// Multiply [`crate::game::tuning::JUMP_BUFFER`] by this (1.0 ..= 1.75).
    pub jump_buffer_mult: f32,
    /// Shrink hazard hitboxes by this many pixels on each side (0 ..= 3).
    pub hitbox_forgiveness_px: f32,
    /// Add one extra checkpoint mid-room/segment.
    pub extra_checkpoint: bool,
    /// Han drops a hint before the room ("mind the drip...").
    pub han_hint: bool,
    /// Extra nuggets placed before quartal (long) gaps, so the band fires up more easily (0 ..= 2).
    pub extra_nuggets_before_quartal: u8,
}

/// Dial at or above which an extra checkpoint is placed.
pub const EXTRA_CHECKPOINT_AT: f32 = 0.35;
/// Dial at or above which Han hints before the room.
pub const HAN_HINT_AT: f32 = 0.55;

impl AssistLevers {
    /// No help at all (dial 0).
    pub const NONE: AssistLevers = AssistLevers {
        coyote_mult: 1.0,
        jump_buffer_mult: 1.0,
        hitbox_forgiveness_px: 0.0,
        extra_checkpoint: false,
        han_hint: false,
        extra_nuggets_before_quartal: 0,
    };

    /// The levers for an assist dial in 0..1 (clamped). Monotone: more dial, never less help.
    pub fn from_dial(dial: f32) -> Self {
        let a = dial.clamp(0.0, 1.0);
        AssistLevers {
            coyote_mult: 1.0 + a,
            jump_buffer_mult: 1.0 + 0.75 * a,
            hitbox_forgiveness_px: (3.0 * a).round(),
            extra_checkpoint: a >= EXTRA_CHECKPOINT_AT,
            han_hint: a >= HAN_HINT_AT,
            extra_nuggets_before_quartal: if a >= 0.7 {
                2
            } else if a >= 0.3 {
                1
            } else {
                0
            },
        }
    }
}

impl Default for AssistLevers {
    fn default() -> Self {
        Self::NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_dial_is_no_help() {
        assert_eq!(AssistLevers::from_dial(0.0), AssistLevers::NONE);
    }

    #[test]
    fn levers_are_monotone_in_the_dial() {
        let mut prev = AssistLevers::from_dial(0.0);
        for i in 1..=20 {
            let l = AssistLevers::from_dial(i as f32 / 20.0);
            assert!(l.coyote_mult >= prev.coyote_mult);
            assert!(l.jump_buffer_mult >= prev.jump_buffer_mult);
            assert!(l.hitbox_forgiveness_px >= prev.hitbox_forgiveness_px);
            assert!(l.extra_checkpoint >= prev.extra_checkpoint);
            assert!(l.han_hint >= prev.han_hint);
            assert!(l.extra_nuggets_before_quartal >= prev.extra_nuggets_before_quartal);
            prev = l;
        }
        assert_eq!(prev.coyote_mult, 2.0);
        assert!(prev.han_hint && prev.extra_checkpoint);
    }

    #[test]
    fn dial_is_clamped() {
        assert_eq!(AssistLevers::from_dial(-1.0), AssistLevers::NONE);
        assert_eq!(AssistLevers::from_dial(5.0), AssistLevers::from_dial(1.0));
    }
}
