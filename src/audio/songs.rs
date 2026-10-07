//! The soundtrack: chunky NES-style arrangements of public-domain jazz standards and rags.
//!
//! Only compositions first published in 1930 or earlier (public domain in the US as of 2026) are
//! used; the arrangements are our own. Composer and year are in each [`Song::title`].
//!
//! Conventions used throughout (they keep the MML unambiguous for the parser):
//! - every bar starts with an absolute `o<n>` and every note has an explicit length, so nothing
//!   depends on carried-over state (repeat bodies and the loop point are state-independent);
//! - a tie `&` is always followed directly by the same pitch, with no commands in between;
//! - pulse1 = lead (`@1`/`@2`, loud), pulse2 = comping / harmony (other duty, quieter),
//!   triangle = walking bass in o1–o2 (no `v`/`@`), noise = swing ride / backbeat + fills.
//!
//! Each `// n:` comment gives bar numbers and chord changes.

use super::{Music, Song};

/// Pick the song for a piece of music. Out-of-range worlds clamp to 1..=5.
pub fn song(music: Music) -> Song {
    match music {
        Music::Title => sweet_georgia_brown(),
        Music::World(0 | 1) => the_entertainer(),
        Music::World(2) => tiger_rag(),
        Music::World(3) => muskrat_ramble(),
        Music::World(4) => st_louis_blues(),
        Music::World(_) => i_got_rhythm(),
        Music::LevelClear => shave_and_a_haircut(),
        Music::Victory => when_the_saints(),
    }
}

// ---------------------------------------------------------------------------------------------
// Shared drum bars (noise channel). Each is exactly one 4/4 bar.

/// Swing ride with kick on 1 and backbeat snare: "ding, ding-a ding, ding-a".
macro_rules! ride {
    () => {
        "k4 s8 h8 h4 s8 h8 "
    };
}
/// Same as [`ride`] but opening a section with an open hat (crash-ish).
macro_rules! ride_open {
    () => {
        "H4 s8 h8 h4 s8 h8 "
    };
}
/// Busy two-beat 8ths (ragtime / dixieland drive).
macro_rules! two_beat {
    () => {
        "k8 h8 s8 h8 k8 h8 s8 h8 "
    };
}
/// Snare roll fill for the last bar of a phrase.
macro_rules! fill {
    () => {
        "k8 s8 s8 s8 s16 s16 s8 k8 s8 "
    };
}
/// A choppier fill with 16th pickups.
macro_rules! fill2 {
    () => {
        "k8 s16 s16 s8 s8 k8 s8 s16 s16 s8 "
    };
}
/// 8-bar swing phrase: crash, six ride bars, fill.
macro_rules! swing8 {
    () => {
        concat!(ride_open!(), "[", ride!(), "]6 ", fill!())
    };
}

// ---------------------------------------------------------------------------------------------
// Title: Sweet Georgia Brown, in F. 32 bars ABAC; each A strain walks the cycle D7-G7-C7-F
// four bars at a time. Brown, swagger, wink.

macro_rules! sgb_mel_a7 {
    () => {
        concat!(
            "o5 r4 d4 e4 f+4 | o5 a2 f+4 d4 | o5 e4 f+4 a4 o6 c4 | o5 a2. r4 | ", // 1-4: D7
            "o5 r4 g4 a4 b4 | o6 d2 o5 b4 g4 | o5 a4 b4 o6 d4 f4 | ",             // 5-7: G7
        )
    };
}
macro_rules! sgb_bass_a7 {
    () => {
        concat!(
            "o2 d4 f+4 a4 o3 c4 | o2 d4 o3 c4 o2 a4 f+4 | o2 d4 e4 f+4 a4 | o2 d4 f+4 a4 a-4 | ",
            "o2 g4 b4 o3 d4 o2 b4 | o2 g4 a4 b4 o3 d4 | o3 f4 e4 d4 o2 b4 | ",
        )
    };
}
macro_rules! sgb_comp_a8 {
    () => {
        concat!(
            "[o4 f+4. f+8 r2 r4 o5 c8 r8 r4 c8 r8]2 ", // 1-4: D7 (3rd / 7th)
            "[o3 b4. b8 r2 r4 o4 f8 r8 r4 f8 r8]2 ",   // 5-8: G7
        )
    };
}

fn sweet_georgia_brown() -> Song {
    Song {
        title: "Sweet Georgia Brown (Bernie/Pinkard/Casey, 1925)",
        bpm: 184.0,
        swing: 0.3,
        looping: true,
        pulse1: concat!(
            "v12 @1 ",
            sgb_mel_a7!(),
            "o6 d2. r4 | ",                                                  // 8: G7
            "o5 r4 c4 d4 e4 | o5 g2 e4 c4 | o5 d4 e4 g4 b-4 | o5 g2. r4 | ", // 9-12: C7
            "o5 r4 c8 d8 f4 a4 | o5 g4. f8 d4 f4 | o5 a2 a8 g8 f8 d8 | o5 c2 r8 c+8 e8 g8 | ", // 13-16: F | F | F | F A7
            sgb_mel_a7!(),
            "o6 d2. r4 | ", // 24: G7
            "o5 r4 d4 f4 a4 | o5 g+4 a4 o6 c+4 o5 a4 | o6 d2 o5 a4 f4 | o5 e2. r4 | ", // 25-28: Dm A7 Dm A7
            "o5 f4 a4 o6 c4 d4 | o5 b4 g4 b-4 g4 | o5 f2 a4 o6 c4 | o6 f4 r4 r2 | ", // 29-32: F D7 | G7 C7 | F | F A7
        ),
        pulse2: concat!(
            "v7 @2 ",
            sgb_comp_a8!(),
            "[o4 e4. e8 r2 r4 b-8 r8 r4 b-8 r8]2 ", // 9-12: C7
            "o4 a4. a8 r2 | o4 r4 f8 r8 r4 f8 r8 | o4 a4. a8 r2 | o4 r4 c+8 r8 r4 g8 r8 | ", // 13-16
            sgb_comp_a8!(),
            "o4 f4. f8 r2 | o4 r4 g8 r8 r4 c+8 r8 | o4 f4. f8 r2 | o4 r4 g8 r8 r4 c+8 r8 | ", // 25-28
            "o4 a4 r8 a8 f+8 r8 o5 c8 r8 | o3 b4 o4 f4 o3 b-4 o4 e4 | o4 a4. a8 r2 | o4 a4 r4 g4 c+4 | ", // 29-32
        ),
        triangle: concat!(
            sgb_bass_a7!(),
            "o2 g4 f4 d4 d-4 | ", // 8: G7 -> C
            "o2 c4 e4 g4 b-4 | o3 c4 o2 b-4 g4 e4 | o2 c4 d4 e4 g4 | o2 b-4 a4 g4 e4 | ", // 9-12
            "o2 f4 a4 o3 c4 o2 a4 | o2 f4 g4 a4 o3 c4 | o3 d4 c4 o2 a4 f4 | o2 f4 c4 o1 a4 o2 c+4 | ", // 13-16
            sgb_bass_a7!(),
            "o2 g4 f4 e4 e-4 | ", // 24: G7 -> Dm
            "o2 d4 f4 a4 f4 | o1 a4 o2 c+4 e4 g4 | o2 d4 e4 f4 a4 | o1 a4 b4 o2 c+4 e4 | ", // 25-28
            "o2 f4 a4 d4 f+4 | o2 g4 b4 c4 e4 | o2 f4 a4 o3 c4 o2 a4 | o2 f4 c4 o1 a4 o2 c+4 | ", // 29-32
        ),
        noise: concat!("v9 [", swing8!(), "]4"),
    }
}

// ---------------------------------------------------------------------------------------------
// World 1 (Bathroom): The Entertainer, in C. The 2/4 A strain with note values doubled into
// 4/4 (one original bar = one bar here), played twice: 2 x 16 bars. A lightly swung, bouncy rag.

macro_rules! ent_1 {
    () => {
        "o4 e8 o5 c4 o4 e8 o5 c4 o4 e8 o5 c8& | "
    };
}
macro_rules! ent_2 {
    () => {
        "c2 c8 d8 d+8 e8 | "
    };
}
macro_rules! ent_3 {
    () => {
        "o5 c8 d8 e4 o4 b8 o5 d4 c8& | "
    };
}

fn the_entertainer() -> Song {
    Song {
        title: "The Entertainer (Scott Joplin, 1902)",
        bpm: 132.0,
        swing: 0.2,
        looping: true,
        pulse1: concat!(
            "v12 @2 [",
            ent_1!(),
            ent_2!(),
            ent_3!(),
            "c4 r4 r4 o4 d8 d+8 | ", // 1-4: C | C | G7 | C
            ent_1!(),
            "c4. o4 a8 g8 f+8 a8 o5 c8 | o5 e4 d8 c8 o4 a8 o5 d4.& | d4 r4 r4 o4 d8 d+8 | ", // 5-8: C | C D7 | G D7 | G G7
            ent_1!(),
            ent_2!(),
            ent_3!(),
            "c4 r8 c8 d8 e8 c8 d8 | ", // 9-12: C | C | G7 | C C7
            "o5 e4 c8 d8 c8 e8 c8 d8 | o5 e4 c8 d8 c8 e8 c8 d8 | o5 e4 o4 b8 o5 d4 c4. | o5 c4 o4 g4 e4 d8 d+8 | ", // 13-16: F | F#dim | C/G G7 | C
            "]2",
        ),
        pulse2: concat!(
            "v7 @0 [",
            "o4 r4 e8 r8 r4 g8 r8 | o4 r4 e8 r8 r4 g8 r8 | o4 r4 f8 r8 r4 b8 r8 | o4 r4 e8 r8 r4 g8 r8 | ",
            "o4 r4 e8 r8 r4 g8 r8 | o4 r4 e8 r8 r4 f+8 r8 | o4 r4 b8 r8 r4 o5 c8 r8 | o4 r4 b8 r8 r4 f8 r8 | ",
            "o4 r4 e8 r8 r4 g8 r8 | o4 r4 e8 r8 r4 g8 r8 | o4 r4 f8 r8 r4 b8 r8 | o4 r4 e8 r8 r4 b-8 r8 | ",
            "o4 r4 a8 r8 r4 o5 c8 r8 | o4 r4 a8 r8 r4 o5 e-8 r8 | o4 r4 e8 r8 r4 f8 r8 | o4 r4 e8 r8 r4 o3 b8 o4 c8 | ",
            "]2",
        ),
        triangle: concat!(
            "[",
            "o2 c4 g4 e4 g4 | o2 c4 e4 f4 f+4 | o2 g4 d4 f4 g4 | o2 c4 g4 e4 g4 | ",
            "o2 c4 g4 e4 g4 | o2 c4 e4 d4 f+4 | o2 g4 b4 d4 f+4 | o2 g4 f4 e4 d4 | ",
            "o2 c4 g4 e4 g4 | o2 c4 e4 f4 f+4 | o2 g4 d4 f4 g4 | o2 c4 e4 g4 e4 | ",
            "o2 f4 c4 a4 c4 | o2 f+4 a4 o3 c4 o2 a4 | o2 g4 e4 g4 d4 | o2 c4 g4 e4 g4 | ",
            "]2",
        ),
        noise: concat!("v8 [[", two_beat!(), "]7 ", fill!(), "]4"),
    }
}

// ---------------------------------------------------------------------------------------------
// World 2 (Pipes): Tiger Rag, in Bb. A busy, clanking 32-bar strain (A A' B A) built around the
// "hold that ti-ger!" tresillo hits (bars 3, 11, 27), with pulse2 growling the answer.

macro_rules! tiger_mel_a7 {
    () => {
        concat!(
            "o5 d8 c+8 d8 f8 r8 d8 r4 | o5 b-8 a8 g8 f8 d8 c8 o4 b-4 | ", // Bb | Bb
            "o5 c4. o4 a4. f4 | o4 a4 o5 c4 e-4 r4 | ", // F7 | F7  hold that ti-ger!
            "o5 c8 o4 b8 o5 c8 e-8 r8 c8 r4 | o5 a8 g+8 a8 g8 f8 e-8 c8 o4 a8 | ", // F7 | F7
            "o4 b-8 o5 c8 d8 f8 r8 d8 o4 b-4 | ",       // Bb
        )
    };
}
macro_rules! tiger_comp_a7 {
    () => {
        concat!(
            "o4 r4 d8 r8 r4 f8 r8 | o4 r4 d8 r8 r4 f8 r8 | ",
            "o4 a4. f4. c4 | o4 r2 a8 a8 f8 r8 | ",
            "o4 r4 e-8 r8 r4 a8 r8 | o4 r4 e-8 r8 r4 a8 r8 | ",
            "o4 r4 d8 r8 r4 f8 r8 | ",
        )
    };
}
macro_rules! tiger_bass_a7 {
    () => {
        concat!(
            "o1 b-4 o2 d4 f4 d4 | o1 b-4 o2 c4 d4 e4 | ",
            "o2 f4 a4 o3 c4 o2 a4 | o2 f4 e-4 c4 o1 a4 | ",
            "o2 f4 a4 o3 c4 o2 a4 | o2 f4 e-4 c4 o1 b4 | ",
            "o1 b-4 o2 d4 f4 d4 | ",
        )
    };
}
macro_rules! tiger_drums8 {
    () => {
        concat!(
            "H8 h8 s8 h8 k8 h8 s8 h8 ",
            two_beat!(),
            "k4. s4. s4 r2 s8 s8 s8 r8 ", // hold that ti-ger! ... (answer)
            "[",
            two_beat!(),
            "]3 ",
            fill2!(),
        )
    };
}

fn tiger_rag() -> Song {
    Song {
        title: "Tiger Rag (Original Dixieland Jass Band, 1917)",
        bpm: 192.0,
        swing: 0.28,
        looping: true,
        pulse1: concat!(
            "v12 @1 ",
            tiger_mel_a7!(),
            "o5 b-4 r4 r4 o4 f8 a8 | ", // 1-8
            tiger_mel_a7!(),
            "o5 b-8 a-8 g8 f8 d8 o4 b-8 o5 c8 d8 | ", // 9-16 (16: Bb7)
            "o5 e-4. g8 r8 b-4. | o5 b-8 a8 b-8 g8 e-8 d8 e-4 | ", // 17-18: Eb | Eb
            "o5 e4. g8 r8 b-4. | o5 b-8 a8 b-8 g8 e8 d+8 e4 | ", // 19-20: Edim | Edim
            "o5 f4. d8 r8 o4 b-4. | o4 b8 o5 d8 f8 g8 r8 f8 d4 | ", // 21-22: Bb/F | G7
            "o5 e8 g8 b-8 g8 e8 c8 d8 e8 | o5 f4 e-4 c4 o4 a4 | ", // 23-24: C7 | F7
            tiger_mel_a7!(),
            "o5 b-4 r4 r4 o4 f8 a8 | ", // 25-32
        ),
        pulse2: concat!(
            "v7 @2 ",
            tiger_comp_a7!(),
            "o4 r4 d8 r8 r4 f8 r8 | ",
            tiger_comp_a7!(),
            "o4 r4 d8 r8 r4 a-8 r8 | ",
            "o4 r4 g8 r8 r4 b-8 r8 | o4 r4 g8 r8 r4 b-8 r8 | ",
            "o4 r4 e8 r8 r4 b-8 r8 | o4 r4 e8 r8 r4 b-8 r8 | ",
            "o4 r4 d8 r8 r4 f8 r8 | o4 r4 f8 r8 r4 b8 r8 | ",
            "o4 r4 e8 r8 r4 b-8 r8 | o4 r4 e-8 r8 r4 a8 r8 | ",
            tiger_comp_a7!(),
            "o4 r4 d8 r8 r4 f8 r8 | ",
        ),
        triangle: concat!(
            tiger_bass_a7!(),
            "o1 b-4 o2 c4 d4 o1 a4 | ",
            tiger_bass_a7!(),
            "o1 b-4 o2 d4 f4 e4 | ",
            "o2 e-4 g4 b-4 g4 | o2 e-4 g4 b-4 f4 | ",
            "o2 e4 g4 b-4 g4 | o2 e4 g4 e4 f+4 | ",
            "o2 f4 d4 o1 b-4 o2 f+4 | o2 g4 b4 d4 d-4 | ",
            "o2 c4 e4 g4 e4 | o2 f4 a4 c4 o1 b4 | ",
            tiger_bass_a7!(),
            "o1 b-4 o2 c4 d4 o1 a4 | ",
        ),
        noise: concat!(
            "v9 ",
            tiger_drums8!(),
            tiger_drums8!(),
            "H8 h8 s8 h8 k8 h8 s8 h8 [",
            two_beat!(),
            "]6 ",
            fill!(),
            tiger_drums8!(),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// World 3 (Sewer): Muskrat Ramble, in Bb. 16 bars of the major ramble, then 16 bars of a sneaky
// G-minor stop-time strain for the rats, turning back to Bb via F7.

macro_rules! musk_bass_bb {
    () => {
        "o1 b-4 o2 d4 f4 d4 | "
    };
}
macro_rules! musk_bass_bb_to_f {
    () => {
        "o1 b-4 o2 c4 d4 e4 | "
    };
}
macro_rules! musk_bass_f7 {
    () => {
        "o2 f4 a4 o3 c4 o2 a4 | "
    };
}
macro_rules! musk_bass_f7_to_bb {
    () => {
        "o2 f4 e-4 c4 o1 b4 | "
    };
}
macro_rules! musk_bass_gm {
    () => {
        "o2 g4 b-4 o3 d4 o2 b-4 | "
    };
}
macro_rules! musk_bass_gm_to_d {
    () => {
        "o2 g4 f4 e4 e-4 | "
    };
}
macro_rules! musk_bass_d7 {
    () => {
        "o2 d4 f+4 a4 f+4 | "
    };
}
macro_rules! musk_bass_d7_to_g {
    () => {
        "o2 d4 f+4 a4 a-4 | "
    };
}
macro_rules! stab {
    (bb) => {
        "o4 r4 d8 r8 r4 f8 r8 | "
    };
    (f7) => {
        "o4 r4 e-8 r8 r4 a8 r8 | "
    };
    (bb7) => {
        "o4 r4 d8 r8 r4 a-8 r8 | "
    };
    (eb) => {
        "o4 r4 g8 r8 r4 b-8 r8 | "
    };
    (edim) => {
        "o4 r4 e8 r8 r4 b-8 r8 | "
    };
    (d7) => {
        "o4 r4 f+8 r8 r4 o5 c8 r8 | "
    };
}
macro_rules! charleston {
    (gm) => {
        "o4 b-4. b-8 r2 | "
    };
    (d7) => {
        "o4 f+4. f+8 r2 | "
    };
    (cm) => {
        "o4 g4. e-8 r2 | "
    };
    (eb) => {
        "o4 g4. g8 r2 | "
    };
}

fn muskrat_ramble() -> Song {
    Song {
        title: "Muskrat Ramble (Kid Ory, 1926)",
        bpm: 168.0,
        swing: 0.32,
        looping: true,
        pulse1: concat!(
            "v12 @1 ",
            "o5 d4 f8 d8 r8 c8 o4 b-4 | o5 c8 d8 r8 f8 r2 | ", // 1-2: Bb | Bb
            "o5 c4 e-8 c8 r8 o4 a8 f4 | o4 a8 b-8 r8 o5 c8 r2 | ", // 3-4: F7 | F7
            "o5 d4 f8 d8 r8 c8 o4 b-4 | o5 c8 d8 r8 f8 r8 g8 f8 d8 | ", // 5-6: Bb | Bb
            "o5 e-4 d8 c8 r8 o4 a8 f4 | o4 f4 a4 o5 c4 e-4 | ", // 7-8: F7 | F7
            "o5 d4. f8 r8 d8 b-4 | o5 a-4 g8 f8 r8 d8 f4 | ",  // 9-10: Bb | Bb7
            "o5 g4. e-8 r8 g8 b-4 | o5 b-4 g8 e8 r8 d-8 e4 | ", // 11-12: Eb | Edim
            "o5 f4 d8 f8 r8 d8 o4 b-4 | o5 c4 e-8 c8 r8 o4 a8 f4 | ", // 13-14: Bb/F | F7
            "o4 b-4 o5 d4 f4 b-4 | o5 c4 o4 a4 f+4 d4 | ",     // 15-16: Bb | D7
            "o4 g8 a8 b-8 b8 o5 c8 c+8 d4 | o5 b-8 a8 g8 r8 d4 r4 | ", // 17-18: Gm | Gm
            "o5 f+8 g8 a8 f+8 d8 c8 o4 a4 | o4 f+4 a4 o5 c4 r4 | ", // 19-20: D7 | D7
            "o4 g8 a8 b-8 b8 o5 c8 c+8 d4 | o5 g8 f8 e-8 d8 c8 o4 b-8 g4 | ", // 21-22: Gm | Gm
            "o5 c4 o4 a8 f+8 r8 d8 e-8 e8 | o4 f+8 g8 r4 o5 d8 c+8 c8 o4 b8 | ", // 23-24: D7 | Gm
            "o5 c4 e-8 c8 r8 g8 e-4 | o5 d4 o4 b-8 g8 r8 d8 g4 | ", // 25-26: Cm | Gm
            "o5 f+8 g8 a8 f+8 d8 c8 o4 a4 | o4 g4 r8 d8 g8 a8 b-8 b8 | ", // 27-28: D7 | Gm
            "o5 e-4 g8 b-8 r8 g8 e-4 | o5 d4 c8 o4 a8 r8 f+8 d4 | ", // 29-30: Eb | D7
            "o4 g8 b-8 o5 d8 g8 e8 c8 o4 b-8 g8 | o4 a4 o5 c4 e-4 r4 | ", // 31-32: Gm C7 | F7
        ),
        pulse2: concat!(
            "v7 @2 ",
            stab!(bb),
            stab!(bb),
            stab!(f7),
            stab!(f7), // 1-4
            stab!(bb),
            stab!(bb),
            stab!(f7),
            stab!(f7), // 5-8
            stab!(bb),
            stab!(bb7),
            stab!(eb),
            stab!(edim), // 9-12
            stab!(bb),
            stab!(f7),
            stab!(bb),
            stab!(d7), // 13-16
            charleston!(gm),
            charleston!(gm),
            charleston!(d7),
            charleston!(d7), // 17-20
            charleston!(gm),
            charleston!(gm),
            charleston!(d7),
            charleston!(gm), // 21-24
            charleston!(cm),
            charleston!(gm),
            charleston!(d7),
            charleston!(gm), // 25-28
            charleston!(eb),
            charleston!(d7),
            "o4 b-4. b-8 e4 r4 | o4 a4. a8 e-4 r4 | ", // 29-32
        ),
        triangle: concat!(
            musk_bass_bb!(),
            musk_bass_bb_to_f!(),
            musk_bass_f7!(),
            musk_bass_f7_to_bb!(), // 1-4
            musk_bass_bb!(),
            musk_bass_bb_to_f!(),
            musk_bass_f7!(),
            musk_bass_f7_to_bb!(), // 5-8
            musk_bass_bb!(),
            "o1 b-4 o2 d4 f4 e4 | o2 e-4 g4 b-4 f4 | o2 e4 g4 b-4 g4 | ", // 9-12
            "o2 f4 d4 o1 b-4 o2 e4 | ",
            musk_bass_f7_to_bb!(),
            "o1 b-4 o2 d4 f4 e-4 | ",
            musk_bass_d7_to_g!(), // 13-16
            musk_bass_gm!(),
            musk_bass_gm_to_d!(),
            musk_bass_d7!(),
            musk_bass_d7_to_g!(), // 17-20
            musk_bass_gm!(),
            musk_bass_gm_to_d!(),
            musk_bass_d7_to_g!(),
            "o2 g4 f4 e-4 d4 | ", // 21-24
            "o2 c4 e-4 g4 f+4 | ",
            musk_bass_gm_to_d!(),
            musk_bass_d7_to_g!(),
            "o2 g4 f4 d4 e4 | ", // 25-28
            "o2 e-4 g4 b-4 e-4 | ",
            musk_bass_d7_to_g!(),
            "o2 g4 b-4 c4 e4 | o2 f4 a4 c4 o1 b4 | ", // 29-32
        ),
        noise: concat!(
            "v9 ",
            swing8!(),
            swing8!(),
            "[k4. s8 r4 h8 h8]7 ",
            fill!(), // stop-time creep
            "[k4. s8 r4 h8 h8]7 ",
            fill2!(),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// World 4 (Septic Tank & Porta-Potty Festival): St. Louis Blues, in G. 12-bar blues, the famous
// 16-bar habanera (tango) strain in G minor, then the 12-bar "St. Louis blues" strain. 40 bars.

macro_rules! stl_bass_blues {
    () => {
        concat!(
            "o2 g4 f4 e4 d4 | o2 c4 e4 g4 f+4 | o2 g4 b4 o3 d4 o2 b4 | o2 g4 f4 e4 d4 | ", // G C7 G G7
            "o2 c4 e4 g4 e4 | o2 c4 e4 g4 f+4 | o2 g4 b4 o3 d4 o2 b4 | o2 g4 f+4 e4 e-4 | ", // C7 C7 G G
            "o2 d4 f+4 a4 d-4 | o2 c4 e4 g4 f+4 | o2 g4 b4 a4 e-4 | o2 d4 f+4 a4 f+4 | ", // D7 C7 G D7
        )
    };
}
macro_rules! stl_comp_blues {
    () => {
        concat!(
            "o4 r4 b8 r8 r4 b8 r8 | o4 r4 b-8 r8 r4 e8 r8 | o4 r4 d8 e8 g8 b-8 b4 | o4 r4 b8 r8 r4 f8 r8 | ",
            "o4 r4 b-8 r8 r4 e8 r8 | o4 r4 b-8 r8 r4 e8 r8 | o4 r4 b8 r8 r4 b8 r8 | o4 r4 b8 r8 r4 b8 r8 | ",
            "o4 r4 c8 r8 r4 f+8 r8 | o4 r4 b-8 r8 r4 e8 r8 | o4 r4 b8 r8 r4 b8 r8 | o4 r4 c8 r8 r4 f+8 r8 | ",
        )
    };
}
macro_rules! habanera {
    (bass gm) => {
        "o2 g8. g16 d8 b-8 g8. g16 d8 b-8 | "
    };
    (bass d7) => {
        "o2 d8. d16 a8 f+8 d8. d16 a8 f+8 | "
    };
    (bass cm) => {
        "o2 c8. c16 g8 e-8 c8. c16 g8 e-8 | "
    };
    (comp gm) => {
        "o4 b-8. b-16 r4 g8. g16 r4 | "
    };
    (comp d7) => {
        "o4 a8. a16 r4 f+8. f+16 r4 | "
    };
    (comp cm) => {
        "o4 g8. g16 r4 e-8. e-16 r4 | "
    };
}
macro_rules! stl_drums_blues {
    () => {
        concat!(ride_open!(), "[", ride!(), "]10 ", fill!())
    };
}

fn st_louis_blues() -> Song {
    Song {
        title: "St. Louis Blues (W. C. Handy, 1914)",
        bpm: 152.0,
        swing: 0.3,
        looping: true,
        pulse1: concat!(
            "v12 @2 ",
            // 1-12: "I hate to see the evening sun go down"
            "o4 r8 g8 a8 b-8 b4 b4 | o4 b-4 a8 g8 e8 g8 r4 | o4 g2. r4 | o4 r2 b8 o5 d8 f8 g8 | ",
            "o4 r8 g8 a8 b-8 o5 c4 c4 | o5 e-8 c8 o4 b-8 a8 g4 e4 | o4 g2 r8 b8 o5 d8 e8 | o5 d4 o4 b4 g4 r4 | ",
            "o5 c4 c4 c8 o4 a8 f+8 a8 | o4 b-8 a8 g8 e8 g4 r4 | o4 g4 b-8 b8 o5 d8 e8 d4 | o5 c4 o4 a4 f+4 d4 | ",
            // 13-28: habanera in G minor
            "o4 g8. a16 b-8 o5 d8 d4 r4 | o5 c4 o4 a8 f+8 d4 r4 | o4 f+8. g16 a8 o5 c8 c4 r4 | o4 b-4 g8 d8 g4 r4 | ",
            "o4 g8. a16 b-8 o5 d8 g4 r4 | o5 f+4 e-8 c8 o4 a4 r4 | o4 a8. b-16 o5 c8 e-8 d4 c4 | o4 b-4 a8 b-8 g4 r4 | ",
            "o5 d4 d8. d16 d8 o4 b-8 g4 | o5 c4 c8. c16 c8 o4 a8 f+4 | o4 a4 b-8 a8 g8 f+8 a4 | o4 g2 r4 r8 o5 d8 | ",
            "o5 e-4 d8 c8 e-4 g4 | o5 d4 o4 b-8 g8 b-4 o5 d4 | o5 c4 o4 a8 f+8 a4 o5 c4 | o5 d4 r4 r8 o4 f+8 g8 a8 | ",
            // 29-40: "Got the St. Louis blues"
            "o5 d4 d8 d8 d8 o4 b8 g4 | o4 b-4 a8 g8 e4 g4 | o4 g8 a8 b-8 b8 o5 d4 g4 | o5 f4 d8 o4 b8 g4 r4 | ",
            "o5 e-4 e-8 e-8 e-8 c8 o4 a4 | o4 b-4 a8 g8 e4 r4 | o4 g8 a8 b-8 b8 o5 d8 e8 d4 | o4 b4 g4 r2 | ",
            "o5 c4 c8 c8 c8 o4 a8 f+4 | o4 b-4 g8 e8 g4 b-4 | o4 g4 b-8 b8 o5 d8 e8 g4 | o5 f+4 d4 c4 o4 a4 | ",
        ),
        pulse2: concat!(
            "v7 @1 ",
            stl_comp_blues!(),
            habanera!(comp gm),
            habanera!(comp d7),
            habanera!(comp d7),
            habanera!(comp gm),
            habanera!(comp gm),
            habanera!(comp d7),
            habanera!(comp d7),
            habanera!(comp gm),
            habanera!(comp gm),
            habanera!(comp d7),
            habanera!(comp d7),
            habanera!(comp gm),
            habanera!(comp cm),
            habanera!(comp gm),
            habanera!(comp d7),
            "o4 a8. a16 r4 f+4 r4 | ",
            stl_comp_blues!(),
        ),
        triangle: concat!(
            stl_bass_blues!(),
            habanera!(bass gm),
            habanera!(bass d7),
            habanera!(bass d7),
            habanera!(bass gm),
            habanera!(bass gm),
            habanera!(bass d7),
            habanera!(bass d7),
            habanera!(bass gm),
            habanera!(bass gm),
            habanera!(bass d7),
            habanera!(bass d7),
            habanera!(bass gm),
            habanera!(bass cm),
            habanera!(bass gm),
            habanera!(bass d7),
            "o2 d8. d16 a8 f+8 d4 f+4 | ",
            stl_bass_blues!(),
        ),
        noise: concat!(
            "v9 ",
            stl_drums_blues!(),
            "H8. h16 s8 h8 k8. h16 s8 h8 [k8. h16 s8 h8 k8. h16 s8 h8]14 ",
            fill2!(),
            stl_drums_blues!(),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// World 5 (Treatment Plant & The Golden Throne): I Got Rhythm, in Bb. A 4-bar royal fanfare, then
// the 32-bar AABA head over rhythm changes; pulse2 answers in the gaps.

macro_rules! igr_mel_a7 {
    () => {
        concat!(
            "o4 f4 g4 b-4 o5 c4& | c2 r2 | ", // Bb G7 | Cm F7   I got rhy-thm
            "o5 c4 o4 b-4 g4 f4& | f2 r2 | ", // Bb G7 | Cm F7   I got mu-sic
            "o4 f4 g4 b-4 o5 c4& | c4 r4 e-4 g-4 | ", // Fm7 Bb7 | Eb Ebm
            "o5 f4 d4 c4 o4 a4 | ",           // Bb F7
        )
    };
}
macro_rules! igr_comp_a7 {
    () => {
        concat!(
            "o4 r4 d8 r8 r4 f8 r8 | o4 r4 e-8 f8 g8 a8 r4 | ",
            "o4 r4 d8 r8 r4 f8 r8 | o4 r4 g8 f8 e-8 c8 r4 | ",
            "o4 r4 a-8 r8 r4 a-8 r8 | o4 r4 g8 r8 r4 g-8 r8 | ",
            "o4 r4 d8 r8 r4 e-8 r8 | ",
        )
    };
}
macro_rules! igr_bass_a7 {
    () => {
        concat!(
            "o1 b-4 o2 d4 g4 d4 | o2 c4 e-4 f4 o1 a4 | ",
            "o1 b-4 o2 d4 g4 d4 | o2 c4 e-4 f4 o1 a4 | ",
            "o2 f4 a-4 b-4 d4 | o2 e-4 g4 g-4 c4 | ",
            "o1 b-4 o2 d4 f4 c4 | ",
        )
    };
}

fn i_got_rhythm() -> Song {
    Song {
        title: "I Got Rhythm (George & Ira Gershwin, 1930)",
        bpm: 192.0,
        swing: 0.3,
        looping: true,
        pulse1: concat!(
            "v12 @1 ",
            "o5 b-4. f8 b-4 o6 d4 | o6 e-4. d8 c4 o5 b-4 | o5 b-8 b-8 b-8 r8 f8 f8 f8 r8 | o5 c2 o4 a4 r4 | ", // intro: Bb Eb Bb F7
            igr_mel_a7!(),
            "o4 b-2 r2 | ",
            igr_mel_a7!(),
            "o4 b-2 r2 | ",
            "o5 f+4. f+8 r8 e8 d4 | o5 c4 o4 a4 f+4 r4 | ", // D7 D7
            "o5 b4. b8 r8 a8 g4 | o5 f4 d4 o4 b4 r4 | ",    // G7 G7
            "o5 e4. e8 r8 d8 c4 | o4 b-4 g4 e4 r4 | ",      // C7 C7
            "o5 a4. a8 r8 g8 f4 | o5 e-4 c4 o4 a4 r4 | ",   // F7 F7
            igr_mel_a7!(),
            "o4 b-4 o5 d4 f4 b-4 | ",
        ),
        pulse2: concat!(
            "v7 @2 ",
            "o5 d4. o4 b-8 o5 d4 f4 | o5 g4. f8 e-4 d4 | o5 f8 f8 f8 r8 d8 d8 d8 r8 | o4 a2 f4 r4 | ",
            igr_comp_a7!(),
            "o4 r4 f8 g8 b-8 o5 d8 r4 | ",
            igr_comp_a7!(),
            "o4 r4 f8 g8 b-8 o5 d8 r4 | ",
            "o4 f+4. f+8 r2 | o4 r4 o5 c8 r8 r4 c8 r8 | ",
            "o4 f4. f8 r2 | o4 r4 b8 r8 r4 b8 r8 | ",
            "o4 e4. e8 r2 | o4 r4 b-8 r8 r4 b-8 r8 | ",
            "o4 e-4. e-8 r2 | o4 r4 a8 r8 r4 a8 r8 | ",
            igr_comp_a7!(),
            "o4 r4 f8 g8 b-8 o5 d8 r4 | ",
        ),
        triangle: concat!(
            "o1 b-4 o2 d4 f4 d4 | o2 e-4 g4 b-4 g4 | o2 f4 d4 o1 b-4 o2 d4 | o2 f4 a4 c4 o1 b4 | ",
            igr_bass_a7!(),
            "o1 b-4 o2 d4 c4 o1 a4 | ",
            igr_bass_a7!(),
            "o1 b-4 o2 d4 f4 e-4 | ",
            "o2 d4 f+4 a4 f+4 | o2 d4 f+4 a4 a-4 | o2 g4 b4 o3 d4 o2 b4 | o2 g4 f4 e4 d4 | ",
            "o2 c4 e4 g4 e4 | o2 c4 e4 g4 e4 | o2 f4 a4 o3 c4 o2 a4 | o2 f4 e-4 c4 o1 b4 | ",
            igr_bass_a7!(),
            "o1 b-4 o2 d4 f4 o1 a4 | ",
        ),
        noise: concat!(
            "v9 ",
            "H4 k4 k4 k4 k4 k4 k4 k4 s8 s8 s8 r8 s8 s8 s8 r8 k4 s8 s8 s16 s16 s16 s16 s4 ",
            swing8!(),
            swing8!(),
            "[H4 s8 h8 H4 s8 h8]7 ",
            fill!(),
            swing8!(),
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// Level clear: a 2-bar "shave and a haircut ... two bits!" button. Doesn't loop.

fn shave_and_a_haircut() -> Song {
    Song {
        title: "Shave and a Haircut (Charles Hale, 1899)",
        bpm: 140.0,
        swing: 0.25,
        looping: false,
        pulse1: "v13 @1 o5 c4 o4 g8 g8 a4 g4 | o4 r4 b4 o5 c2",
        pulse2: "v8 @2 o4 e4 e8 e8 f4 e4 | o4 r4 f4 e2",
        triangle: "o2 c4 r4 f4 e4 | o2 r4 o1 g4 o2 c2",
        noise: "v10 k4 s8 s8 k4 s4 | r4 s4 H2",
    }
}

// ---------------------------------------------------------------------------------------------
// Victory / credits: When the Saints Go Marching In, in F. Two 16-bar choruses: a stately one,
// then a hot one an octave up with Charleston comping.

fn when_the_saints() -> Song {
    Song {
        title: "When the Saints Go Marching In (traditional)",
        bpm: 168.0,
        swing: 0.3,
        looping: true,
        pulse1: concat!(
            "v12 @2 ",
            "o4 r4 f4 a4 b-4 | o5 c1 | o4 r4 f4 a4 b-4 | o5 c1 | ", // 1-4: F
            "o4 r4 f4 a4 b-4 | o5 c2 o4 a2 | o4 f2 a2 | o4 g1 | ",  // 5-8: F F F C7
            "o4 r4 a4 a4 g4 | o4 f2. a4 | o5 c2 c4 o4 b-4& | b-2 r2 | ", // 9-12: F F7 Bb Bbm
            "o4 a4 b-4 o5 c4 o4 a4 | o4 f2 g2 | o4 f1 | o4 r1 | ",  // 13-16: F C7 F F
            "o5 r4 f8 g8 a4 b-4 | o6 c2 r4 o5 a8 o6 c8 | o5 r4 f8 g8 a4 b-4 | o6 c2. r4 | ", // 17-20
            "o5 r4 f8 g8 a4 b-4 | o6 c4. o5 a8 r8 a4. | o5 f4. a8 r8 g4. | o5 g4 e4 c4 e4 | ", // 21-24
            "o5 r4 a4 a8 a8 g4 | o5 f4. e-8 r8 f8 a4 | o6 c4 c8 c8 c4 o5 b-4 | o5 b-4 a-8 f8 d-4 r4 | ", // 25-28
            "o5 a4 b-4 o6 c4 o5 a4 | o5 f4. g8 r8 e8 c4 | o5 f4 a4 o6 c4 f4 | o6 f4 r4 r2 | ", // 29-32
        ),
        pulse2: concat!(
            "v7 @1 ",
            "o4 r4 c4 f4 g4 | o4 a1 | o4 r4 c4 f4 g4 | o4 a1 | ",
            "o4 r4 c4 f4 g4 | o4 a2 f2 | o4 c2 f2 | o4 e1 | ",
            "o4 r4 f4 f4 e4 | o4 c2. e-4 | o4 f2 f4 d4 | o4 d-2 r2 | ",
            "o4 f4 g4 a4 f4 | o4 c2 e2 | o4 c1 | o4 r4 c8 d8 e8 g8 b-4 | ",
            "[o4 a4. a8 r4 c4]7 ", // 17-23: F
            "o4 b-4. b-8 r4 g4 | o4 a4. a8 r4 c4 | o4 a4. a8 r4 e-4 | o4 b-4. b-8 r4 d4 | o4 d-4. d-8 r4 f4 | ",
            "o4 a4. a8 r4 c4 | o4 b-4. b-8 r4 e4 | o4 a4. a8 r4 c4 | o4 c8 f8 a8 o5 c8 f4 r4 | ",
        ),
        triangle: concat!(
            "o2 [",
            "[o2 f4 a4 o3 c4 o2 a4 o2 f4 c4 d4 e4]3 ", // 1-6: F
            "o2 f4 e4 d4 d-4 | o2 c4 e4 g4 e4 | ",     // 7-8: F C7
            "o2 f4 a4 o3 c4 o2 a4 | o2 f4 a4 o3 c4 o2 a4 | ", // 9-10: F F7
            "o2 b-4 o3 d4 f4 d4 | o2 b-4 o3 d-4 o2 f4 g4 | ", // 11-12: Bb Bbm
            "o2 f4 a4 d4 d-4 | o2 c4 e4 g4 e4 | o2 f4 a4 o3 c4 o2 a4 | o2 f4 c4 d4 e4 | ", // 13-16
            "]2",
        ),
        noise: concat!(
            "v9 ",
            "H4 s8 h8 k4 s8 h8 [k4 s8 h8 k4 s8 h8]6 ",
            fill!(),
            "[k4 s8 h8 k4 s8 h8]7 ",
            fill!(),
            "H8 h8 s8 h8 k8 h8 s8 h8 [",
            two_beat!(),
            "]6 ",
            fill2!(),
            "[",
            two_beat!(),
            "]7 s16 s16 s16 s16 s8 s8 k8 s8 H4",
        ),
    }
}

#[cfg(test)]
mod tests {
    //! A self-contained MML checker mirroring the dialect in `audio/mod.rs`: it measures track
    //! lengths (independently of the real parser) and rejects anything outside the conventions.

    use super::*;

    /// Duration units: a whole note is 192, so a dotted 32nd (9) is still whole.
    const WHOLE: u64 = 192;
    const BAR: u64 = WHOLE; // 4/4

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Chan {
        Pulse,
        Triangle,
        Noise,
    }

    struct Checker<'a> {
        src: &'a [u8],
        i: usize,
        chan: Chan,
        octave: Option<i32>,
        default_len: Option<u64>,
        oct_range: (i32, i32),
    }

    impl Checker<'_> {
        fn err(&self, msg: &str) -> String {
            let lo = self.i.saturating_sub(20);
            let hi = (self.i + 20).min(self.src.len());
            format!(
                "{msg} at byte {} near `{}`",
                self.i,
                String::from_utf8_lossy(&self.src[lo..hi])
            )
        }

        fn skip_ws(&mut self) {
            while self.i < self.src.len()
                && matches!(self.src[self.i], b' ' | b'\n' | b'\t' | b'\r' | b'|')
            {
                self.i += 1;
            }
        }

        fn number(&mut self) -> Option<u64> {
            let start = self.i;
            while self.i < self.src.len() && self.src[self.i].is_ascii_digit() {
                self.i += 1;
            }
            (self.i > start).then(|| {
                std::str::from_utf8(&self.src[start..self.i])
                    .unwrap()
                    .parse()
                    .unwrap()
            })
        }

        /// Optional length + optional dot, in units.
        fn length(&mut self) -> Result<u64, String> {
            let base = match self.number() {
                Some(n) => {
                    if ![1, 2, 4, 8, 16, 32].contains(&n) {
                        return Err(self.err(&format!("bad length {n}")));
                    }
                    WHOLE / n
                }
                None => {
                    WHOLE
                        / self
                            .default_len
                            .ok_or_else(|| self.err("note without length and no `l` set"))?
                }
            };
            if self.src.get(self.i) == Some(&b'.') {
                self.i += 1;
                if base % 2 != 0 {
                    return Err(self.err("dot makes a fractional length"));
                }
                return Ok(base * 3 / 2);
            }
            Ok(base)
        }

        /// Parses until end of input or a `]`; returns the duration in units.
        fn seq(&mut self, in_repeat: bool) -> Result<u64, String> {
            let mut total = 0;
            loop {
                self.skip_ws();
                let Some(&c) = self.src.get(self.i) else {
                    return if in_repeat {
                        Err(self.err("unclosed `[`"))
                    } else {
                        Ok(total)
                    };
                };
                let melodic = self.chan != Chan::Noise;
                match c {
                    b']' => {
                        return if in_repeat {
                            Ok(total)
                        } else {
                            Err(self.err("stray `]`"))
                        };
                    }
                    b'[' => {
                        self.i += 1;
                        let before = self.octave;
                        // A body that starts with an absolute `o` is state-independent.
                        self.skip_ws();
                        let starts_absolute = self.src.get(self.i) == Some(&b'o');
                        let body = self.seq(true)?;
                        self.i += 1; // `]`
                        let n = self
                            .number()
                            .ok_or_else(|| self.err("`]` without repeat count"))?;
                        if n == 0 {
                            return Err(self.err("repeat count 0"));
                        }
                        if melodic && !starts_absolute && self.octave != before {
                            return Err(self.err("repeat body changes octave"));
                        }
                        total += body * n;
                    }
                    b'a'..=b'g' if melodic => {
                        self.i += 1;
                        if matches!(self.src.get(self.i), Some(b'+' | b'#' | b'-')) {
                            self.i += 1;
                        }
                        let oct = self.octave.ok_or_else(|| self.err("note before any `o`"))?;
                        if oct < self.oct_range.0 || oct > self.oct_range.1 {
                            return Err(self
                                .err(&format!("octave {oct} out of range {:?}", self.oct_range)));
                        }
                        total += self.length()?;
                        self.skip_ws();
                        if self.src.get(self.i) == Some(&b'&') {
                            self.i += 1;
                            self.skip_ws();
                            if !matches!(self.src.get(self.i), Some(b'a'..=b'g')) {
                                return Err(self.err("`&` not followed directly by a note"));
                            }
                        }
                    }
                    b'k' | b's' | b'h' | b'H' if !melodic => {
                        self.i += 1;
                        total += self.length()?;
                    }
                    b'r' => {
                        self.i += 1;
                        total += self.length()?;
                    }
                    b'o' if melodic => {
                        self.i += 1;
                        self.octave = Some(
                            self.number()
                                .ok_or_else(|| self.err("`o` without number"))?
                                as i32,
                        );
                    }
                    b'<' | b'>' if melodic => {
                        self.i += 1;
                        let o = self
                            .octave
                            .ok_or_else(|| self.err("relative octave before `o`"))?;
                        self.octave = Some(if c == b'>' { o + 1 } else { o - 1 });
                    }
                    b'l' => {
                        self.i += 1;
                        let n = self
                            .number()
                            .ok_or_else(|| self.err("`l` without number"))?;
                        if ![1, 2, 4, 8, 16, 32].contains(&n) {
                            return Err(self.err("bad `l`"));
                        }
                        self.default_len = Some(n);
                    }
                    b'v' if self.chan != Chan::Triangle => {
                        self.i += 1;
                        let n = self
                            .number()
                            .ok_or_else(|| self.err("`v` without number"))?;
                        if n > 15 {
                            return Err(self.err("volume > 15"));
                        }
                    }
                    b'@' if self.chan == Chan::Pulse => {
                        self.i += 1;
                        let n = self
                            .number()
                            .ok_or_else(|| self.err("`@` without number"))?;
                        if n > 3 {
                            return Err(self.err("duty > 3"));
                        }
                    }
                    _ => {
                        return Err(
                            self.err(&format!("unexpected `{}` on {:?}", c as char, self.chan))
                        );
                    }
                }
            }
        }
    }

    /// Duration of a track in units (whole note = 192), or a description of what's wrong.
    fn measure(src: &str, chan: Chan, oct_range: (i32, i32)) -> Result<u64, String> {
        Checker {
            src: src.as_bytes(),
            i: 0,
            chan,
            octave: None,
            default_len: None,
            oct_range,
        }
        .seq(false)
    }

    fn tracks(s: &Song) -> [(&'static str, &'static str, Chan, (i32, i32)); 4] {
        [
            ("pulse1", s.pulse1, Chan::Pulse, (3, 6)),
            ("pulse2", s.pulse2, Chan::Pulse, (3, 6)),
            ("triangle", s.triangle, Chan::Triangle, (1, 3)),
            ("noise", s.noise, Chan::Noise, (0, 0)),
        ]
    }

    #[test]
    fn checker_counts_like_the_spec() {
        let p = |s| measure(s, Chan::Pulse, (0, 9)).unwrap();
        assert_eq!(p("o4 c4 e-8 g+16. a32"), 48 + 24 + 18 + 6);
        assert_eq!(p("o4 l8 c d4 e"), 24 + 48 + 24);
        assert_eq!(p("o4 [c4 [d8]2 ]3 r1"), 3 * 96 + 192);
        assert_eq!(p("o4 c4&c16 | > c2. <"), 48 + 12 + 144);
        assert_eq!(
            measure("k8 h8 s8 H8 r2 [k16]4", Chan::Noise, (0, 0)).unwrap(),
            4 * 24 + 96 + 48
        );
        assert!(measure("o4 c4", Chan::Noise, (0, 0)).is_err());
        assert!(measure("k4", Chan::Pulse, (0, 9)).is_err());
        assert!(measure("o4 c3", Chan::Pulse, (0, 9)).is_err());
        assert!(measure("o4 [c4 >]2", Chan::Pulse, (0, 9)).is_err());
        assert!(measure("o4 c4& o5 c4", Chan::Pulse, (0, 9)).is_err());
        assert!(measure("@1 o2 c4", Chan::Triangle, (1, 3)).is_err());
        assert!(measure("o2 c4", Chan::Pulse, (3, 6)).is_err());
    }

    #[test]
    fn every_song_is_well_formed_and_aligned() {
        for music in Music::ALL {
            let s = song(music);
            let mut lens = Vec::new();
            for (name, src, chan, range) in tracks(&s) {
                let len =
                    measure(src, chan, range).unwrap_or_else(|e| panic!("{music:?} {name}: {e}"));
                lens.push((name, len));
            }
            let (_, first) = lens[0];
            for &(name, len) in &lens {
                assert_eq!(
                    len,
                    first,
                    "{music:?}: {name} is {} beats but pulse1 is {} beats",
                    len as f64 / 48.0,
                    first as f64 / 48.0
                );
            }
            assert!(
                first > 0 && first % BAR == 0,
                "{music:?}: {first} units is not whole 4/4 bars"
            );
            let bars = first / BAR;
            let secs = (first / 48) as f32 * 60.0 / s.bpm;
            println!("{music:?}: {bars} bars, {secs:.1}s");
            if s.looping {
                assert!(
                    (30.0..=100.0).contains(&secs),
                    "{music:?}: loop is {secs:.1}s ({bars} bars)"
                );
                assert!((120.0..=200.0).contains(&s.bpm), "{music:?}: bpm {}", s.bpm);
            } else {
                assert!(
                    (2.0..=6.0).contains(&secs),
                    "{music:?}: jingle is {secs:.1}s"
                );
            }
            assert!(
                (0.0..=0.4).contains(&s.swing),
                "{music:?}: swing {}",
                s.swing
            );
        }
    }

    #[test]
    fn only_the_level_clear_jingle_is_one_shot() {
        for music in Music::ALL {
            assert_eq!(song(music).looping, music != Music::LevelClear, "{music:?}");
        }
    }

    #[test]
    fn titles_credit_public_domain_sources() {
        let mut seen = std::collections::HashSet::new();
        for music in Music::ALL {
            let title = song(music).title;
            assert!(seen.insert(title), "{title} used twice");
            let credit = title
                .rsplit_once('(')
                .map(|(_, c)| c)
                .unwrap_or_else(|| panic!("{title}: no credit"));
            if credit.starts_with("traditional") {
                continue;
            }
            let year: u32 = credit
                .trim_end_matches(')')
                .rsplit(", ")
                .next()
                .unwrap()
                .parse()
                .unwrap_or_else(|_| panic!("{title}: no year"));
            assert!(
                year <= 1930,
                "{title}: {year} is not public domain in the US"
            );
        }
    }

    #[test]
    fn out_of_range_worlds_still_have_music() {
        assert_eq!(song(Music::World(0)).title, song(Music::World(1)).title);
        assert_eq!(song(Music::World(9)).title, song(Music::World(5)).title);
    }
}
