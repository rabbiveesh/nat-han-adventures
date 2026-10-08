//! The `.song` grammar as data: one place for the editor's cheat sheet (and these docs).
//! `tests/live.rs` checks that every token the parsers accept is listed here.

/// `(section, [(syntax, meaning)])`, in reading order.
pub const CHEAT_SHEET: &[(&str, &[(&str, &str)])] = &[
    (
        "Song file",
        &[
            ("[song]", "header: title, bpm, swing, key, loop, meter (one `name = value` a line)"),
            ("title = Tiger Rag (1917)", "shown in the credits"),
            ("bpm = 184", "tempo, quarter notes per minute"),
            ("swing = 0.3", "0 straight .. 0.33 triplet swing: off-beat 8ths start late"),
            ("key = F", "home key's tonic (`C`, `Bb`, `F#`, or 0..11): tunings are relative to it"),
            ("loop = yes", "`yes`/`no` (`true`/`false`): loop at the end of the longest track"),
            ("meter = 4/4", "beats per bar / beat unit (default 4/4); every `|` is checked against it"),
            ("[chords]", "the chord chart, bar by bar (needed for the reharmonizing filters)"),
            ("[instruments]", "FamiTracker-style instruments, drum kits and each channel's palette (optional)"),
            ("[pulse1]", "lead melody (pulse wave, slightly left)"),
            ("[pulse2]", "comping / harmony (pulse wave, slightly right)"),
            ("[triangle]", "bass (the 4-bit NES triangle)"),
            ("[noise]", "drums"),
            ("; a comment", "`;` starts a comment, to the end of the line, anywhere"),
        ],
    ),
    (
        "Notes",
        &[
            ("c d e f g a b", "notes (melodic channels, lower case)"),
            ("c+ c#", "sharp"),
            ("b-", "flat"),
            ("r", "rest"),
            ("{c e g}8", "chord played as a fast arpeggio (up to 6 notes, lowest first)"),
            ("{a > c e}", "`<` `>` inside braces shift the octave for the rest of the chord only"),
        ],
    ),
    (
        "Lengths",
        &[
            ("c4 c8 c16", "1/n note: 1 whole, 2 half, 4 quarter ... any n in 1..=96 (c12 = 8th triplet)"),
            ("c4.", "dotted: x1.5 (`c4..` = 4 + 8 + 16); needs an explicit length"),
            ("l8", "default length for notes without one"),
            ("c4&c16", "tie: one longer note; to a different pitch it's a slur (no re-attack)"),
        ],
    ),
    (
        "Octave, volume, duty",
        &[
            ("o4", "octave 0..8 (o4 c = middle C)"),
            (">", "octave up"),
            ("<", "octave down"),
            ("v12", "volume 0..15"),
            ("@1", "pulse duty: @0 12.5%, @1 25%, @2 50%, @3 75%"),
            ("@i brass", "play instrument `brass` from here on (can change mid-line); `@i default` = built-in"),
        ],
    ),
    (
        "Structure",
        &[
            ("|", "bar line: checked, must fall exactly on a bar line of the meter"),
            ("[c d e f |]3", "repeat the bracketed part 3 times (2 without a number); nestable"),
            ("t120", "not supported: tempo is `bpm` in [song]"),
        ],
    ),
    (
        "Drums ([noise])",
        &[("k", "kick"), ("s", "snare"), ("h", "closed hi-hat"), ("H", "open hi-hat (choked by the next hit)"), ("x", "crash cymbal")],
    ),
    (
        "Instruments ([instruments])",
        &[
            ("lead : vol 15 12 | 9 8 | duty 2", "a tone instrument: `name : macros`; each step is one 60 Hz frame"),
            ("vol 15 12 9 | 8 / 4 0", "volume 0..15 x the note's `v`; `|` loops from here, `/` starts the release"),
            ("duty 0 1 2", "pulse duty per frame (overrides `@n`)"),
            ("pitch +3 +1 0", "offset in semitones per frame (+0.5 = 50 cents); not cumulative"),
            ("arp | 0 4 7", "the same as pitch: a looping arpeggio"),
            ("vib delay=8 depth=12 speed=5.5 ramp=15", "vibrato: delay/ramp in frames, depth in cents, speed in Hz"),
            ("fade 0.8", "the built-in pulses' gentle decay to 65% (seconds)"),
            ("tri", "the triangle's 4-bit wave (on any channel)"),
            ("| between macros", "optional separator (`|` before a keyword); a sequence without `|` holds its last step"),
            ("kit : kick pitch=-24 decay=6 | snare noise=short", "a drum kit: `drum key=value` for kick snare hat ohat crash"),
            ("kick pitch=-24 hz=48 sweep=1.5 decay=4 click=0.3", "kick: sweep start (semitones), bottom Hz, sweep and decay (frames), click 0..1"),
            ("snare noise=short period=5 decay=3 tone=185 body=2", "snare: NES noise mode and period 0..15, rattle decay, tone Hz and its decay"),
            ("hat decay=1 | ohat decay=5 | crash period=1 noise=long", "hats and crash: decay (frames), NES noise period 0..15, noise mode"),
            ("pulse1 = default brass pluck", "a channel's palette: what its musician may switch to (`default` = built-in)"),
            ("bossa.pulse1 = clarinet flute", "a feel's palette (bossa samba rock funk): what the band plays it on; unlisted = built-ins"),
        ],
    ),
    (
        "Chord chart ([chords])",
        &[
            ("| D7 | % | G7 C7 | F6 Dm7 Gm7 C7 |", "a bar holds 1, 2 or 4 chords splitting it evenly"),
            ("%", "repeat the previous chord (a whole bar or one slot)"),
            ("C7/E", "slash bass"),
            ("Bb F#", "root `A`-`G`, then `#` or `b`"),
            ("C 6 maj7 7 9 7b9 7#9 7#5 7sus4", "major qualities and dominants"),
            ("m m6 m7 mMaj7 m7b5 dim7 aug", "minor, half-diminished, diminished, augmented"),
        ],
    ),
];

/// The `[song]` keys.
pub const SONG_KEYS: [&str; 6] = ["title", "bpm", "swing", "key", "loop", "meter"];

/// The section names, in file order.
pub const SECTIONS: [&str; 7] = ["song", "chords", "instruments", "pulse1", "pulse2", "triangle", "noise"];

/// The cheat sheet as plain text (for terminals and docs).
pub fn cheat_sheet_text() -> String {
    let mut s = String::new();
    for (section, rows) in CHEAT_SHEET {
        s += &format!("{section}\n");
        for (syntax, meaning) in *rows {
            s += &format!("  {syntax:<36} {meaning}\n");
        }
    }
    s
}
