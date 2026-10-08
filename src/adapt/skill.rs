//! What a room exercises, and how hard.

/// Difficulty band of one skill: 1 (gentlest) ..= 10 (hardest).
pub type Band = u8;
pub const MIN_BAND: Band = 1;
pub const MAX_BAND: Band = 10;

/// One thing a room can ask of the player. Each has its own band, spread and window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Skill {
    /// Jumping and platforming: gap widths, ledge sizes, jump timing.
    Precision,
    /// Flies and air-freshener sprays.
    HazardTiming,
    MovingPlatforms,
    /// Giant walls (climbable under Giant Steps).
    GiantSteps,
    /// Long gaps (jumpable while the band is fired up).
    FiredUp,
    /// Waltz rows.
    Waltz,
    /// Death-mechanic rooms: some rooms expect you to die (stains).
    Stains,
    /// Grease chutes with sweaty grip.
    Grease,
    /// Han's gates: buddy ledges, buddy raft pools, shield rows, chain chasms.
    Buddy,
}

impl Skill {
    pub const ALL: [Skill; 9] = [
        Skill::Precision,
        Skill::HazardTiming,
        Skill::MovingPlatforms,
        Skill::GiantSteps,
        Skill::FiredUp,
        Skill::Waltz,
        Skill::Stains,
        Skill::Grease,
        Skill::Buddy,
    ];
    pub const COUNT: usize = Self::ALL.len();

    /// Index into per-skill arrays (`ALL[s.index()] == s`).
    pub fn index(self) -> usize {
        self as usize
    }

    /// Short lowercase name (simulator output, debug logs). Never shown to the player.
    pub fn name(self) -> &'static str {
        match self {
            Skill::Precision => "precision",
            Skill::HazardTiming => "hazard",
            Skill::MovingPlatforms => "platforms",
            Skill::GiantSteps => "giant",
            Skill::FiredUp => "fired",
            Skill::Waltz => "waltz",
            Skill::Stains => "stains",
            Skill::Grease => "grease",
            Skill::Buddy => "buddy",
        }
    }
}

/// Clamp any integer into `MIN_BAND..=MAX_BAND`.
pub fn clamp_band(b: i32) -> Band {
    b.clamp(MIN_BAND as i32, MAX_BAND as i32) as Band
}
