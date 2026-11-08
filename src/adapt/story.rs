//! Story mode: the hand-made levels don't change, but the same silent assists do.
//!
//! No bands. Each level gives an expectation of deaths per checkpoint segment; deaths beyond it
//! raise the dial ([`STORY_RISE`] each), and every segment cleared within it fades the dial
//! ([`STORY_FADE`]). Frustration ([`super::frustration`]) adds [`STORY_FRUSTRATION`] once per
//! segment and asks Han to encourage. The output is the same [`AssistLevers`] as free play.
//! Nothing here is ever shown to the player.

use super::assist::AssistLevers;
use super::frustration::{self, FrustrationSignal};

pub const STORY_RISE: f32 = 0.08;
pub const STORY_FADE: f32 = 0.12;
pub const STORY_FRUSTRATION: f32 = 0.15;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StoryAssist {
    /// The assist dial, 0..1. Carries across levels (save it with progress).
    pub assists: f32,
    /// Deaths a checkpoint segment of this level is expected to cost.
    pub expected_deaths: u32,
    /// Deaths since the last checkpoint (or level start).
    pub segment_deaths: u32,
    /// Restarts of this level.
    pub restarts: u32,
    /// Frustration already handled in this segment.
    pub segment_frustrated: bool,
    /// What the last event asks of the caller.
    pub encourage: Option<FrustrationSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StoryEvent {
    LevelStarted { expected_deaths_per_segment: u32 },
    Died,
    /// Idled this long right after a death.
    IdleAfterDeath { secs: f32 },
    CheckpointReached,
    LevelRestarted,
    LevelCompleted,
}

impl StoryAssist {
    /// Start from an existing dial (e.g. the free-play profile's, or the saved one).
    pub fn new(assists: f32) -> Self {
        StoryAssist { assists: assists.clamp(0.0, 1.0), ..Default::default() }
    }

    pub fn levers(&self) -> AssistLevers {
        AssistLevers::from_dial(self.assists)
    }
}

/// The next story-assist state after `event`. Pure, like [`super::reduce`].
pub fn reduce_story(state: StoryAssist, event: StoryEvent) -> StoryAssist {
    let s = StoryAssist { encourage: None, ..state };
    match event {
        StoryEvent::LevelStarted { expected_deaths_per_segment } => StoryAssist {
            expected_deaths: expected_deaths_per_segment,
            segment_deaths: 0,
            restarts: 0,
            segment_frustrated: false,
            ..s
        },
        StoryEvent::Died => {
            let segment_deaths = s.segment_deaths + 1;
            let excess = segment_deaths.saturating_sub(s.expected_deaths);
            let rise = if excess > 0 { STORY_RISE } else { 0.0 };
            let s = StoryAssist { segment_deaths, assists: (s.assists + rise).min(1.0), ..s };
            frustrate(s, frustration::detect(excess, 0.0, 0))
        }
        StoryEvent::IdleAfterDeath { secs } => frustrate(s, frustration::detect(0, secs, 0)),
        StoryEvent::LevelRestarted => {
            let restarts = s.restarts + 1;
            let s = StoryAssist { restarts, segment_deaths: 0, segment_frustrated: false, ..s };
            frustrate(s, frustration::detect(0, 0.0, restarts))
        }
        StoryEvent::CheckpointReached | StoryEvent::LevelCompleted => {
            let fade = if s.segment_deaths <= s.expected_deaths { STORY_FADE } else { 0.0 };
            StoryAssist {
                assists: (s.assists - fade).max(0.0),
                segment_deaths: 0,
                segment_frustrated: false,
                ..s
            }
        }
    }
}

fn frustrate(s: StoryAssist, signal: Option<FrustrationSignal>) -> StoryAssist {
    match signal {
        Some(signal) if !s.segment_frustrated => StoryAssist {
            assists: (s.assists + STORY_FRUSTRATION).min(1.0),
            segment_frustrated: true,
            encourage: Some(signal),
            ..s
        },
        _ => s,
    }
}
