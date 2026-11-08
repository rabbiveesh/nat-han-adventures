//! A built-in exercise with a chord chart, so the reharmonizing filters can be heard (and
//! tested) independently of the soundtrack: 16 bars of ii–V–Is down in whole steps
//! (C, Bb, Db, C), the classic Countdown / Giant Steps playground. Not used in the game.

use super::live::SongFile;

pub fn demo_song() -> SongFile {
    let pulse1 = concat!(
        "v12 @1 ",
        "o5 d4 f4 a4 o6 c4 | o5 b4 a4 g4 f4 | o5 e2 g4 b4 | o5 e2. r4 | ",
        "o5 c4 e-4 g4 b-4 | o5 a4 g4 f4 e-4 | o5 d2 f4 a4 | o5 d2. r4 | ",
        "o5 e-4 g-4 b-4 o6 d-4 | o6 c4 o5 b-4 a-4 g-4 | o5 f2 a-4 o6 c4 | o5 f2. r4 | ",
        "o5 d8 e8 f8 a8 o6 c4 o5 a4 | o5 b8 a8 g8 f8 e4 d4 | o5 e2 g2 | o5 f4 a4 g4 f4 | ",
    );
    let pulse2 = concat!(
        "v7 @2 ",
        "o4 f1 | o4 f1 | o4 e1 | o4 e1 | o4 e-1 | o4 e-1 | o4 d1 | o4 d1 | ",
        "o4 g-1 | o4 g-1 | o4 f1 | o4 f1 | o4 f1 | o4 f1 | o4 e1 | o4 f2 f2 | ",
    );
    let triangle = concat!(
        "o2 d4 f4 a4 f4 | o2 g4 b4 o3 d4 o2 b4 | o2 c4 e4 g4 e4 | o2 c4 g4 e4 c4 | ",
        "o2 c4 e-4 g4 e-4 | o2 f4 a4 o3 c4 o2 a4 | o2 b-4 o3 d4 f4 d4 | o2 b-4 f4 d4 o1 b-4 | ",
        "o2 e-4 g-4 b-4 g-4 | o2 a-4 o3 c4 e-4 c4 | o2 d-4 f4 a-4 f4 | o2 d-4 a-4 f4 d4 | ",
        "o2 d4 f4 a4 f4 | o2 g4 b4 o3 d4 o2 b4 | o2 c4 e4 g4 e4 | o2 d4 a4 g4 b4 | ",
    );
    let noise = "v9 H4 s8 h8 h4 s8 h8 [k4 s8 h8 h4 s8 h8]14 k8 s8 s8 s8 s16 s16 s8 k8 s8";
    let chords = concat!(
        "| Dm7 | G7 | Cmaj7 | % | Cm7 | F7 | Bbmaj7 | % ",
        "| Ebm7 | Ab7 | Dbmaj7 | % | Dm7 | G7 | Cmaj7 | Dm7 G7 |",
    );
    SongFile::from_mml("ii-V-I Exercise (demo)", 176.0, 0.3, true, 0, chords, [pulse1, pulse2, triangle, noise]).expect("the demo parses")
}
