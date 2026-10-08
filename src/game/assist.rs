//! Invisible assists: the levers the adaptive engine ([`crate::adapt`]) turns. Gameplay reads
//! them; nothing here is ever shown to the player. Every lever only *loosens* things relative
//! to the defaults, which are what the level validator proves solvable (the floors).
//!
//! The adaptive wiring writes this resource; physics, hazards and Han read it.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Assists>();
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct Assists {
    /// × [`super::tuning::COYOTE_TIME`] (≥ 1).
    pub coyote_mult: f32,
    /// × [`super::tuning::JUMP_BUFFER`] (≥ 1).
    pub jump_buffer_mult: f32,
    /// Extra px of forgiveness on hazard hitboxes (≥ 0).
    pub hitbox_forgiveness_px: f32,
    /// × raft lifetimes (≥ 1; the floor is what the validator uses).
    pub raft_life_mult: f32,
    /// How eager Han is to help, 0 (lazy) ..= 1 (very eager): how soon he braces, intercepts,
    /// goes ahead into hazards, and how forgiving his overuse limit is. 0.5 = neutral.
    pub han_eagerness: f32,
}

impl Default for Assists {
    fn default() -> Self {
        Self {
            coyote_mult: 1.0,
            jump_buffer_mult: 1.0,
            hitbox_forgiveness_px: 0.0,
            raft_life_mult: 1.0,
            han_eagerness: 0.5,
        }
    }
}

impl Assists {
    /// Coyote time (s): never below [`super::tuning::COYOTE_TIME`].
    pub fn coyote_time(&self) -> f32 {
        super::tuning::COYOTE_TIME * self.coyote_mult.max(1.0)
    }

    /// Jump buffer (s): never below [`super::tuning::JUMP_BUFFER`].
    pub fn jump_buffer(&self) -> f32 {
        super::tuning::JUMP_BUFFER * self.jump_buffer_mult.max(1.0)
    }

    /// Extra hazard-hitbox forgiveness (px per side), never negative.
    pub fn extra_forgiveness(&self) -> f32 {
        if self.hitbox_forgiveness_px.is_finite() { self.hitbox_forgiveness_px.max(0.0) } else { 0.0 }
    }
}
