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
    /// Side B's tunes, 1..=[`Music::SIDE_B_TUNES`] ([`Music::side_b`] picks one).
    SideB(u8),
}

impl Music {
    pub const ALL: [Music; 12] = [
        Music::Title,
        Music::World(1),
        Music::World(2),
        Music::World(3),
        Music::World(4),
        Music::World(5),
        Music::LevelClear,
        Music::Victory,
        Music::SideB(1),
        Music::SideB(2),
        Music::SideB(3),
        Music::SideB(4),
    ];

    /// How many tunes Side B has.
    pub const SIDE_B_TUNES: u8 = 4;
    const SIDE_B_LEVELS: usize = 10;

    /// The tune for Side B level `level` (0-based): its levels in runs, a tune to each run, in
    /// order. A free play Side B room (`room`, the `level`th of the run) takes them in turn.
    pub fn side_b(level: usize, room: bool) -> Music {
        let n = Self::SIDE_B_TUNES as usize;
        let tune = if room { level % n } else { (level * n / Self::SIDE_B_LEVELS).min(n - 1) };
        Music::SideB(tune as u8 + 1)
    }

    /// Lower-case name for files ("title", "world3", ...).
    pub fn slug(self) -> String {
        match self {
            Music::World(w) => format!("world{w}"),
            Music::SideB(t) => format!("sideb{t}"),
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
    /// The laughing band's boing boost: Han's head is a spring.
    Boing,
    /// The encore's last cymbal crash (and a big drum): it knocks Nat into the throne.
    Crash,
    /// A level's secret found (`game::secret`).
    Secret,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_b_levels_take_the_tunes_in_runs_and_rooms_in_turn() {
        let levels: Vec<Music> = (0..10).map(|l| Music::side_b(l, false)).collect();
        let n = Music::SIDE_B_TUNES;
        assert_eq!(levels[0], Music::SideB(1));
        assert_eq!(levels[9], Music::SideB(n));
        assert!(levels.windows(2).all(|w| matches!((w[0], w[1]), (Music::SideB(a), Music::SideB(b)) if b == a || b == a + 1)));
        for t in 1..=n {
            assert!(levels.contains(&Music::SideB(t)), "tune {t} never plays");
            assert!(Music::ALL.contains(&Music::SideB(t)));
        }
        assert_eq!(Music::side_b(0, true), Music::SideB(1));
        assert_eq!(Music::side_b(n as usize, true), Music::SideB(1));
        assert_eq!(Music::side_b(1, true), Music::SideB(2));
    }
}
