//! Unproductive struggle, as opposed to the productive kind.
//!
//! Dying is part of a platformer; dying *a lot more than the room expects*, sitting still after
//! a splat, or restarting the same room over and over are the signals that the player is no
//! longer having fun. Free play and story mode share these thresholds.

/// Why we think the player is frustrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrustrationSignal {
    /// [`EXCESS_DEATHS`]+ deaths beyond what the room expects.
    ManyDeaths,
    /// Idle for more than [`IDLE_SECS`] right after a death.
    IdleAfterDeath,
    /// Restarted the same room [`RESTARTS`]+ times.
    Restarts,
}

/// Deaths beyond expected, in one room, that count as frustration.
pub const EXCESS_DEATHS: u32 = 3;
/// Seconds idle after a death that count as frustration.
pub const IDLE_SECS: f32 = 15.0;
/// Restarts of one room that count as frustration.
pub const RESTARTS: u32 = 3;

/// The first signal that fires, if any. `idle_after_death_secs` is the longest stretch with no
/// input right after a death.
pub fn detect(excess_deaths: u32, idle_after_death_secs: f32, restarts: u32) -> Option<FrustrationSignal> {
    if excess_deaths >= EXCESS_DEATHS {
        Some(FrustrationSignal::ManyDeaths)
    } else if idle_after_death_secs > IDLE_SECS {
        Some(FrustrationSignal::IdleAfterDeath)
    } else if restarts >= RESTARTS {
        Some(FrustrationSignal::Restarts)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds() {
        assert_eq!(detect(2, 15.0, 2), None);
        assert_eq!(detect(3, 0.0, 0), Some(FrustrationSignal::ManyDeaths));
        assert_eq!(detect(0, 15.1, 0), Some(FrustrationSignal::IdleAfterDeath));
        assert_eq!(detect(0, 0.0, 3), Some(FrustrationSignal::Restarts));
    }
}
