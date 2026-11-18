#!/usr/bin/env python3
"""How each soloist gets into a strong beat, what it aims at there, and how a phrase moves.

The numbers behind the soloist's `target_weights`, `devices` and `modes` (`soloist.rs`,
`music/players/*.player`), after Josh Walsh's reading of Oscar Peterson: a target on a strong
beat, a way into it, a way out. For each of the band's measured soloists and the whole
Weimar Jazz Database v2.1:

  1. targets: the degree of each strong-beat note (beats 1 and 3, on the beat, inside a phrase,
     not its first note) by the chord's family: a chord tone, a tension or chromatic; over a
     dominant, plain (R 3 5 7), a natural tension (9 #11 13) or altered (b9 #9 b13), and the
     same over dominants that go to a minor chord;
  2. devices: how the line gets into that note from the notes just before it (each within a
     beat of the next): an enclosure (one side then the other, the last a step off), a scale
     run (2+ notes stepping one way into it), a blues run (the same in the key's blues scale,
     with a minor 3rd step or a blue note in it), an arpeggio (2+ notes by 3rds and 4ths one
     way, chord tones), a half step, a whole step, a leap, or nothing (a rest first);
  3. modes: each phrase of 6+ notes as over the changes, scalar (70%+ of its moves by step) or
     blues (85%+ of its notes in the key's blues scale with a blue note among them).

The engine's own lines can be read the same way (`--notes`, a TSV of solo, phrase, beat, pitch,
chord, key), so its weights can be set until it matches the records. Statistics only: nothing
here copies a phrase. Standard library only:

  python3 -I scripts/analysis/wjazzd_devices.py wjazzd.db > results/devices.txt
  python3 -I scripts/analysis/wjazzd_devices.py wjazzd.db --json results/devices.json
  python3 -I scripts/analysis/wjazzd_devices.py --notes engine.tsv     # the engine's lines
  python3 -I scripts/analysis/wjazzd_devices.py --selftest             # no database
"""

import argparse
import collections
import json
import os
import re
import sqlite3
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wjazzd_phrase_endings as E  # noqa: E402

DEVICES = ["enclosure", "scale run", "blues run", "arpeggio", "half step", "whole step", "leap", "after a rest"]
TARGETS_DOM = ["plain", "natural tension", "altered", "chromatic"]
TARGETS = ["chord tone", "tension", "chromatic"]
MODES = ["changes", "scalar", "blues"]
NATURAL = {2, 6, 9}
ALTERED = {1, 3, 8}
BLUES = (0, 3, 5, 6, 7, 10)
IN_KEY = {0, 2, 3, 5, 7, 10}  # chord roots the key's blues scale covers (the engine's rule)
SCALAR, BLUESY = 0.70, 0.85


def parse_key(s):
    """A WJazzD key (`Bb-maj`, `C-min`, `Eb`, `F-chrom`) as (tonic pitch class, minor?)."""
    m = re.match(r"\s*([A-G])([b#]?)(?:-?(maj|min|[a-z]+))?", s or "")
    if not m:
        return None
    pc = (E.NOTE[m.group(1)] + {"b": -1, "#": 1, "": 0}[m.group(2)]) % 12
    return pc, m.group(3) == "min"


def blues_scale(key, chord):
    """The blues scale a line plays over this chord (the engine's `blues_scale`): the key's
    when the chord's root sits in it (I ii bIII IV V bVII), else the chord's own (a major
    chord's relative minor's)."""
    root, fam, _ = chord
    if key is not None and (root - key[0]) % 12 in IN_KEY:
        t = key[0]
    else:
        t = (root + 9) % 12 if fam == "maj" else root
    return {(t + i) % 12 for i in BLUES}, t


def target_kind(pitch, chord):
    root, fam, tones = chord
    i = (pitch - root) % 12
    if fam == "dom":
        if i in tones:
            return "plain"
        return "natural tension" if i in NATURAL else "altered" if i in ALTERED else "chromatic"
    _, kind = E.degree(pitch, chord)
    return {"ct": "chord tone", "tension": "tension"}.get(kind, "chromatic")


def device(before, t, chord, scale):
    """How the notes `before` (nearest last; each within a beat of the next) get into `t`."""
    if not before:
        return "after a rest"
    a = before[-1]
    if len(before) >= 2:
        b = before[-2]
        if (a - t) * (b - t) < 0 and 1 <= abs(a - t) <= 2 and abs(b - t) <= 3:
            return "enclosure"
    # The run into it: the longest stretch moving one way, by steps or by 3rds and 4ths.
    seq = before[-4:] + [t]
    ivs = [y - x for x, y in zip(seq, seq[1:])]
    d = 1 if ivs[-1] > 0 else -1 if ivs[-1] < 0 else 0
    n = 0
    for iv in reversed(ivs):
        if iv * d <= 0:
            break
        n += 1
    run, steps = seq[-n - 1:], [abs(i) for i in ivs[-n:]]
    if n >= 2 and d:
        pcs = {p % 12 for p in run[:-1]}
        blue = {(scale[1] + 3) % 12, (scale[1] + 6) % 12}
        if all(p % 12 in scale[0] for p in run) and all(s <= 3 for s in steps) and (3 in steps or pcs & blue):
            return "blues run"
        if all(s <= 2 for s in steps):
            return "scale run"
        if all(3 <= s <= 5 for s in steps) and all((p - chord[0]) % 12 in chord[2] for p in run[:-1]):
            return "arpeggio"
    step = abs(a - t)
    return "half step" if step == 1 else "whole step" if step == 2 else "leap"


def mode(phrase, key):
    """changes, scalar or blues: a phrase as (pitch, chord) pairs."""
    ps = [p for p, _ in phrase]
    ivs = [abs(y - x) for x, y in zip(ps, ps[1:]) if y != x]
    if not ivs:
        return "changes"
    inside, blue = 0, False
    for p, c in phrase:
        scale = blues_scale(key, c)
        if p % 12 in scale[0]:
            inside += 1
            blue |= (p - scale[1]) % 12 in (6,) or key is not None and not key[1] and (p - scale[1]) % 12 == 3
    if inside / len(phrase) >= BLUESY and blue:
        return "blues"
    if sum(1 for i in ivs if i <= 2) / len(ivs) >= SCALAR:
        return "scalar"
    return "changes"


def resolves_minor(chords, k):
    """The chord a dominant at `k` goes to (the next different one, within 8 beats) is minor."""
    c = chords[k]
    for j in range(k + 1, min(k + 9, len(chords))):
        n = chords[j]
        if n and (n[0], n[1]) != (c[0], c[1]):
            return n[1] in ("min", "half")
    return False


# --- one solo, as phrases of notes on a beat grid ----------------------------------------


def tally(phrases, key):
    """phrases: lists of (beat, pitch, chord, strong, next_minor); the counts."""
    t = collections.Counter()
    for ph in phrases:
        if len(ph) >= 6:
            t["mode", mode([(p, c) for _, p, c, _, _ in ph], key)] += 1
        for k, (beat, pitch, chord, strong, minor) in enumerate(ph):
            if not strong or k == 0:
                continue
            fam = "dom" if chord[1] == "dom" else "other"
            kind = target_kind(pitch, chord)
            t["target", fam, kind] += 1
            if fam == "dom" and minor:
                t["target", "dom to minor", kind] += 1
            before = []
            j, at = k - 1, beat
            while j >= 0 and len(before) < 4 and at - ph[j][0] <= 1.0 + 1e-6:
                before.insert(0, ph[j][1])
                at = ph[j][0]
                j -= 1
            if before and beat - ph[k - 1][0] > 1.0 + 1e-6:
                before = []
            t["device", device(before, pitch, chord, blues_scale(key, chord))] += 1
    return t


def from_db(db, melid, key):
    notes, beats, secs = E.load(db, melid)
    if not notes or len(beats) < 2:
        return []
    grid = E.Beats(beats)
    ends = set(E.phrase_ends(notes, secs))
    starts = set()
    if secs:
        st = [s for s, _ in secs]
        if max(st) < len(notes):
            starts = set(st)
        else:
            ids = {n[0]: i for i, n in enumerate(notes)}
            starts = {ids[s] for s in st if s in ids}
    phrases, cur = [], []
    for k, (_, onset, dur, pitch, bar, beat, tatum, division) in enumerate(notes):
        if k in starts and cur:
            phrases.append(cur)
            cur = []
        i = grid.index(onset)
        if i >= 0 and grid.chord[i] is not None and pitch:
            blen = grid.beat_len(i) or 0.5
            at = i + (onset - grid.on[i]) / blen
            strong = beat in (1, 3) and E.place(tatum, division) == "on"
            minor = grid.chord[i][1] == "dom" and resolves_minor(grid.chord, i)
            cur.append((at, pitch, grid.chord[i], strong, minor))
        if k in ends:
            phrases.append(cur)
            cur = []
    if cur:
        phrases.append(cur)
    return phrases


def solo_keys(db):
    if "key" not in E.columns(db, "solo_info"):
        print("warning: solo_info has no key; blues scales go by each chord's root", file=sys.stderr)
        return {}
    return {m: parse_key(k) for m, k in db.execute("SELECT melid, key FROM solo_info")}


def analyse(db):
    E.need(db, "melody", ["eventid", "melid", "onset", "duration", "pitch", "bar", "beat", "tatum", "division"])
    E.need(db, "beats", ["melid", "onset", "bar", "beat", "chord", "form"])
    rows, keys = E.solos(db), solo_keys(db)
    cache, out = {}, {}
    for slug, perf, y0, y1, feel in E.PLAYERS:
        t = collections.Counter()
        ids = E.pick(rows, perf, y0, y1, feel)
        for melid in ids:
            if melid not in cache:
                cache[melid] = tally(from_db(db, melid, keys.get(melid)), keys.get(melid))
            t += cache[melid]
        out[slug] = summarise(t, len(ids))
    return out


def from_tsv(path):
    """solo, phrase, beat, pitch, chord, key per line (# comments): the engine's lines."""
    solos = collections.defaultdict(lambda: collections.defaultdict(list))
    keys = {}
    with open(path) as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            solo, phrase, beat, pitch, chord, key = line.rstrip("\n").split("\t")[:6]
            c = E.parse_chord(chord)
            if c:
                b = float(beat)
                solos[solo][phrase].append([b, int(pitch), c, abs(b - round(b)) < 1e-6 and round(b) % 2 == 0, False])
                keys[solo] = parse_key(key)
    out = {}
    for solo, phrases in solos.items():
        t = collections.Counter()
        for ph in phrases.values():
            chords = [n[2] for n in ph]
            for k, n in enumerate(ph):
                n[4] = n[2][1] == "dom" and resolves_minor(chords, k)
        t += tally([[tuple(n) for n in ph] for ph in phrases.values()], keys[solo])
        out[solo] = summarise(t, 1)
    return out


# --- the report ---------------------------------------------------------------------------


def pct(c, keys):
    n = sum(c.get(k, 0) for k in keys)
    return {k: round(100.0 * c.get(k, 0) / n, 1) if n else None for k in keys}, n


def summarise(t, solos):
    s = {"solos": solos}
    s["targets_other"], s["n_other"] = pct({k: t["target", "other", k] for k in TARGETS}, TARGETS)
    s["targets_dom"], s["n_dom"] = pct({k: t["target", "dom", k] for k in TARGETS_DOM}, TARGETS_DOM)
    s["targets_dom_to_minor"], s["n_dom_to_minor"] = pct({k: t["target", "dom to minor", k] for k in TARGETS_DOM}, TARGETS_DOM)
    s["devices"], s["n_devices"] = pct({k: t["device", k] for k in DEVICES}, DEVICES)
    s["modes"], s["n_phrases"] = pct({k: t["mode", k] for k in MODES}, MODES)
    d = s["targets_dom"]
    tens = (d["natural tension"] or 0) + (d["altered"] or 0)
    s["dom_tensions"] = round(tens / 100, 2)
    s["altered_of_tensions"] = round(d["altered"] / tens, 2) if tens else None
    return s


def row(d, keys):
    return "  ".join(f"{k} {'-' if d[k] is None else d[k]}" for k in keys)


def report(results):
    print("Targets, devices and modes in the Weimar Jazz Database v2.1, by player. Strong beats: 1 and 3,")
    print("on the beat, inside a phrase (not its first note). Shares in %.\n")
    for slug, s in results.items():
        print(f"{slug}: {s['solos']} solos")
        print(f"  targets, major/minor/other (n={s['n_other']})  {row(s['targets_other'], TARGETS)}")
        print(f"  targets, dominant (n={s['n_dom']})             {row(s['targets_dom'], TARGETS_DOM)}")
        print(f"  targets, dominant to minor (n={s['n_dom_to_minor']})    {row(s['targets_dom_to_minor'], TARGETS_DOM)}")
        print(f"    as .player: dom_tensions {s['dom_tensions']}  altered {s['altered_of_tensions']}")
        print(f"  devices into them (n={s['n_devices']})  {row(s['devices'], DEVICES)}")
        print(f"  phrase modes (n={s['n_phrases']}, 6+ notes)  {row(s['modes'], MODES)}")
        print()


# --- a self-test on made-up phrases -------------------------------------------------------


def selftest():
    c7, f7, dm = E.parse_chord("C7"), E.parse_chord("F7"), E.parse_chord("D-7")
    blues_c = blues_scale((0, False), c7)
    assert blues_c == ({0, 3, 5, 6, 7, 10}, 0), blues_c
    assert blues_scale((0, False), E.parse_chord("Ab7"))[1] == 8  # not in the key: its own
    assert blues_scale((0, False), E.parse_chord("Ebj7"))[1] == 0  # bIII: the key's
    assert target_kind(64, c7) == "plain" and target_kind(62, c7) == "natural tension" and target_kind(61, c7) == "altered"
    assert target_kind(65, dm) == "chord tone" and target_kind(64, dm) == "tension"
    assert device([], 64, c7, blues_c) == "after a rest"
    assert device([65, 63], 64, c7, blues_c) == "enclosure"
    assert device([60, 62], 64, c7, blues_c) == "scale run"
    assert device([67, 66, 65], 64, c7, blues_c) == "scale run"  # chromatic steps: a scale run
    assert device([60, 63, 65], 67, c7, blues_c) == "blues run"  # C Eb F G: by a minor 3rd
    assert device([48, 52, 55], 58, c7, blues_c) == "arpeggio"  # C E G into Bb
    assert device([63], 64, c7, blues_c) == "half step" and device([71], 64, c7, blues_c) == "leap"
    assert mode([(p, c7) for p in (60, 62, 64, 65, 67, 69, 70)], (0, False)) == "scalar"
    assert mode([(p, c) for p, c in ((60, c7), (63, c7), (65, f7), (66, f7), (67, c7), (70, c7))], (0, False)) == "blues"
    assert mode([(p, c7) for p in (60, 64, 67, 70, 64, 72)], (0, False)) == "changes"
    # A phrase: rest, then into beat 2 (strong) by a scale run and beat 4 by a half step.
    ph = [(0.0, 60, c7, False, False), (0.5, 62, c7, False, False), (1.0, 63, c7, False, False),
          (2.0, 64, c7, True, False), (3.5, 66, f7, False, False), (4.0, 65, f7, True, False)]
    t = tally([ph], (0, False))
    assert t["device", "scale run"] == 1 and t["device", "half step"] == 1, t
    assert t["target", "dom", "plain"] == 2, t
    print("selftest: ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("db", nargs="?", help="wjazzd.db")
    ap.add_argument("--json", help="also write the numbers here")
    ap.add_argument("--notes", help="read the engine's lines from this TSV instead")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if a.notes:
        results = from_tsv(a.notes)
    elif a.db:
        results = analyse(sqlite3.connect(a.db))
    else:
        ap.error("give wjazzd.db, --notes or --selftest")
    report(results)
    if a.json:
        with open(a.json, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    main()
