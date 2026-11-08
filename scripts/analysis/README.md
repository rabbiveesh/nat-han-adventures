# Analysis scripts

One-off measurements behind the band's numbers (`crates/bandstand/music/players/*.player`, `crates/bandstand/music/bass/`,
`pocket.rs`, `mix.rs`, `groove_ms`). None of them run in the game or its tests. They print
statistics only. No audio or data dumps are kept here.

## Phrase endings

`wjazzd_phrase_endings.py`: where each soloist ends a phrase, by target beat and target degree,
reading through anticipations and chromatic last notes. `--selftest` needs no data; `--schema`
lists the database's tables.

```bash
python3 -I scripts/analysis/wjazzd_phrase_endings.py <wjazzd.db> --json scripts/analysis/results/phrase-endings.json > scripts/analysis/results/phrase-endings.txt
```

Results are in `results/`. Phrases with no chord at their last note are dropped and counted
(3.8% over all solos, mostly tunes with almost no chord symbols, like Wayne Shorter's "Orbits").

## Inputs (none of them in this repo)

- **`wjazzd.db`**: the [Weimar Jazz Database](https://jazzomat.hfm-weimar.de/download/downloads/wjazzd.db), v2.1.
- **`DATA_DIR` for `funk_bass.py`**: Lakh MIDI (LMD-matched), the MSD Last.fm tags, `match_scores.json`, `unique_tracks.txt`.
- **Groove MIDI Dataset** for `groove_timing.py`.
- **Demucs stems** (`htdemucs`, `htdemucs_6s`) of your own copies of the records, for `mix.py` and the bass passes. These need a venv with demucs, torch, madmom, torchcrepe, librosa and pretty_midi.

## `laptop/`: the earlier scripts

Gathered from the laptop on 2026-11-05. Each one takes the database path as its argument unless
noted.

Many of these lived in temp folders that were deleted after their runs, so they were rebuilt from
the session logs: the first version plus every later patch, in order. "Rebuilt, matches" means its
output today is byte-identical to the saved report from the time. The rest were copied from disk.

### Weimar Jazz DB: soloists

| File | Measured | Source |
|---|---|---|
| `analyze_first_pass.py` | Tempo per performer: stability, trend, 8-bar turn | Rebuilt, runs (no saved report) |
| `wjd_soloists.py` | Per-group density, phrase lengths and rests, anticipations, rhythm classes, chord tones | Rebuilt, runs (no saved report) |
| `an_dynamics_motifs.py` | Dynamics (pitch vs loudness), motif sharing between phrases | Rebuilt, runs (no saved report) |
| `runs_motifs.py` | Fast runs: share of notes, length, speed; metric placement | Rebuilt, runs (no saved report) |
| `licks.py` | Each player's recurring interval patterns | Rebuilt, matches `durhay-licks-report.txt` |
| `licks_originals.py` | The same, with changes to the method (the "originals" report) | Rebuilt, matches `durhay-licks-report-originals.txt` |
| `armstrong.py` | Louis Armstrong: full profile, dynamics, licks | Rebuilt, matches `durhay-armstrong.txt` |
| `funk_latin_profile.py` | The funk and Latin solos as `.player` profiles | Rebuilt, matches `durhay-funk-latin-soloists.txt` |

### Weimar Jazz DB: timing and swing

| File | Measured | Source |
|---|---|---|
| `soloist_timing.py` | Lay-back against the annotated beats, swing ratio, by player and tempo | Rebuilt, matches the main part of `durhay-soloist-timing.txt` |
| `drift.py` | The same question with a wider window (notes within 1/4 beat) | Rebuilt, nearly matches that report's appendix (its header differs) |
| `rhythm_feel_timing.py` | Fusion and post-bop solos; timing and 16ths by rhythm feel | Rebuilt, runs from the log only |

### Walking bass

| File | Measured | Source |
|---|---|---|
| `walking.py` | Beat-wise `bass_pitch` stats by group and bassist (the walkers) | Rebuilt, all its lines are in `durhay-walking-bass.txt` |
| `walking_leaps_feel.py` | Leaps, anticipations, two-feel bars, repeating cells per bassist | From the log |
| `chambers_rhythm.py` | Paul Chambers' rhythm inside the beat. Input: Demucs bass stems | From disk |
| `sunny_twofeel.py` | Tommy Bryant's two-feel on "On the Sunny Side of the Street". Input: a bass stem | From disk |
| `sunny_pairs.py` | Bryant's paired quarters vs walking. Input: a bass stem | From disk |
| `stem_cleanliness.py` | How clean that bass stem is: bleed, pitch clarity | From the log |

### Mix, drums, funk bass

| File | Measured | Source |
|---|---|---|
| `mix.py` | Rhythm-section level under the soloist (the ducking). Input: Demucs stems | From disk |
| `groove_timing.py` | Per-style drum timing from the Groove MIDI Dataset | From disk |
| `funk_bass.py` | Funk, soul and disco bass lines from Lakh MIDI, selected by Last.fm tags | From disk |

## Not here

- **Jazz Trio Database timing** (`pocket.rs`): no script found on the laptop or in its session logs.
- **The transcription kit** (`~/personal/durhay-transcribe`: `prep.sh`, `song_setup.sh`, `export.sh`, `scripts/`): a tool for learning tunes, not an analysis, so it stays there.
- **Smaller one-off snippets from the transcription sessions** (scoring first-pass notes against the answer key, tempo and bar-phase checks): these exist only in the session logs.

- `wjazzd_phrase_starts.py`: the phrase-endings script's partner for each phrase's first note (beat, pickups, arrival, degrees, first move).
- `wjazzd_phrase_start_slots.py`: adds to it the exact start slot (1, the "and" of 1, 2 ...), the first interval's size, the leap from the last phrase's end and rests in buckets.
- `wjazzd_strategies.py`: how a solo is planned, to check the soloist's choices: the arc over each chorus and solo, restated ideas and their transposition, the notes on chord changes and phrase contours, 16ths and notes per bar by tempo, and phrase starts after 8-bar lines. Results in `results/strategies.txt`.
- `wjazzd_reharm.py`: how each soloist implies reharmonisation, so the band can reharm to suit them: degree classes over dominants, tritone-sub fits, anticipating the next chord, superimposed ii-Vs, side-slips, major-3rd triad runs, and outside notes over minor and major. Each has a chance baseline. Results in `results/reharm.txt`.
- `wjazzd_style.py`: style beyond the notes. Covers swing, lay-back and jitter by tempo; register across choruses; phrase edges and held anticipations against the changes; density through an 8-bar section and the turnaround; steps, leaps and runs by tempo; dynamics by register, phrase and chorus; legato and loudness-ghosted off-beats. Results in `results/style.txt`.
- `wjazzd_more.py`: the remaining habits. Covers blue notes, tensions resolving by step, hemiola and 3-note contour groupings, the soloist against the database's bass notes, long notes' degrees, and phrase length and rest by tempo. Each has a chance baseline. Results in `results/more.txt`.
