//! What the music is, with no game in it: which piece ([`Music`]), how the band plays it
//! ([`Filters`]: a [`Harmony`] and the laughing band's tuning), and the sound effects ([`Sfx`]).
//! The engine ([`super::live`]), the director and the offline tools all speak these; only the
//! Bevy plugin ([`super::plugin`]) knows about the game.

use bevy::reflect::Reflect;

/// Which piece of music to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Music {
    Title,
    /// Level music for world 1..=5.
    World(u8),
    /// Short fanfare on reaching the goal (doesn't loop).
    LevelClear,
    /// After level 10: credits music.
    Victory,
}

impl Music {
    pub const ALL: [Music; 8] = [
        Music::Title,
        Music::World(1),
        Music::World(2),
        Music::World(3),
        Music::World(4),
        Music::World(5),
        Music::LevelClear,
        Music::Victory,
    ];

    /// Lower-case name for files ("title", "world3", ...).
    pub fn slug(self) -> String {
        match self {
            Music::World(w) => format!("world{w}"),
            other => format!("{other:?}").to_lowercase(),
        }
    }
}

/// "Filters" chosen by the [`super::director`] from how the player is doing, to make the music
/// clunkier and stranger. See [`super::theory`] for the music theory, [`super::accomp`] for the
/// generated accompaniment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub struct Filters {
    pub harmony: Harmony,
    /// The laughing band: retune everything to [`super::tuning::Tuning::Medley`] (a different
    /// tuning every phrase — just intonation, harmonic series, 7-TET, Carlos alpha,
    /// Bohlen–Pierce — relative to the song's key, slightly drunk on top). The name is
    /// historical: it started out as plain 5-limit just intonation, which
    /// [`super::tuning::Tuning::Just`] still forces.
    pub just_intonation: bool,
}

/// How the song is harmonized: new pulse 2 + triangle ([`super::accomp`]), the melody following
/// ([`super::melody`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum Harmony {
    /// As written.
    #[default]
    Original,
    /// Coltrane changes: ii-V-I / V-I resolutions become Giant Steps cycles through major thirds.
    Coltrane,
    /// McCoy Tyner: quartal voicings (stacked fourths), pounding root-fifth left hand.
    Quartal,
    /// Every chord replaced by a melodic minor sonority (altered, lydian dominant, mMaj7, ...).
    MelodicMinor,
    /// A jazz waltz: the song re-cut into 3/4 ([`super::waltz`]), oom-pah-pah. Longer than the
    /// others.
    Waltz,
}

impl Harmony {
    pub const ALL: [Harmony; 5] =
        [Harmony::Original, Harmony::Coltrane, Harmony::Quartal, Harmony::MelodicMinor, Harmony::Waltz];

    /// Short upper-case label ("" for the original).
    pub fn label(self) -> &'static str {
        match self {
            Harmony::Original => "",
            Harmony::Coltrane => "COLTRANE CHANGES",
            Harmony::Quartal => "QUARTAL",
            Harmony::MelodicMinor => "MELODIC MINOR",
            Harmony::Waltz => "JAZZ WALTZ",
        }
    }

    /// Lower-case name for files and the `NATHAN_MUSIC` override.
    pub fn slug(self) -> &'static str {
        match self {
            Harmony::Original => "original",
            Harmony::Coltrane => "coltrane",
            Harmony::Quartal => "quartal",
            Harmony::MelodicMinor => "melodic",
            Harmony::Waltz => "waltz",
        }
    }
}

impl Filters {
    /// "COLTRANE CHANGES", "TUNING? WHAT TUNING", "QUARTAL + TUNING? WHAT TUNING", or "" when plain.
    pub fn label(&self) -> String {
        const TUNING: &str = "TUNING? WHAT TUNING";
        match (self.harmony.label(), self.just_intonation) {
            (h, false) => h.to_string(),
            ("", true) => TUNING.to_string(),
            (h, true) => format!("{h} + {TUNING}"),
        }
    }

    /// Parse `coltrane`, `quartal`, `melodic`, `waltz`, `original`, each optionally `+ji`, or just
    /// `ji` (case-insensitive). `None` if not understood.
    pub fn parse(s: &str) -> Option<Filters> {
        let mut f = Filters::default();
        for part in s.to_ascii_lowercase().split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "ji" => f.just_intonation = true,
                p => {
                    f.harmony = *Harmony::ALL
                        .iter()
                        .find(|h| h.slug() == p || (p == "melodicminor" && **h == Harmony::MelodicMinor))?
                }
            }
        }
        Some(f)
    }
}

/// Sound effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum Sfx {
    Jump,
    /// Double jump: a short comedic toot.
    Toot,
    Land,
    Nugget,
    Splat,
    Checkpoint,
    /// Toilet flush on reaching the goal.
    Flush,
    MenuMove,
    MenuSelect,
    /// Han talking: a little "blip blip" babble.
    HanBlip,
}
