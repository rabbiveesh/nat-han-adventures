#!/usr/bin/env python3
"""How our soloists imply reharmonisation: what the band could reharm to suit its soloist.

Reads the Weimar Jazz Database v2.1 for each of the band's measured soloists and the whole
database, and measures, each against a chance baseline:

  1. dominant 7ths: the notes (all, and on beats 1 and 3) by degree class: plain (R 3 5 b7),
     natural tensions (9 13), altered (b9 #9 #11 b13), other (11, maj 7); and the share of
     dominant spans whose notes fit the tritone sub's chord tones better than the written
     chord's (chance: the same spans' notes moved by an unrelated interval);
  2. anticipation: notes in the last beat (and the last half beat) before a chord change
     that fit the next chord but not the current one (chance: the same test on notes two to
     three beats before the change);
  3. a superimposed ii-V: over a dominant lasting a bar or more, the first half fitting the
     ii a 5th above and the second half the V itself; over a one-chord bar of another kind,
     the ii-V of the next bar's chord (chance: the same halves the other way round, V then ii);
  4. side-slipping: 4-note runs mostly outside the chord's scale and inside the scale a half
     step up or down, as a share of bars (chance: the same test a whole step away);
  5. multi-tonic: over static harmony (a chord held a bar or more), 4-note runs fitting
     1-2-3-5 of a major triad a major 3rd from the root, as a share of such spans (chance: the
     same test on the other roots, two at a time, averaged; roots inside the chord's scale
     make it generous);
  6. minor 7ths and major chords: notes outside the chord's scale (Dorian, Ionian), and the
     #11 over major (chance: the share of the octave outside the scale, and 1/12).

Chord scales: dominant Mixolydian, minor Dorian, major Ionian, half-diminished Locrian,
diminished whole-half, augmented whole-tone. Statistics only: nothing here copies a phrase.
Standard library only:

  python3 -I scripts/analysis/wjazzd_reharm.py wjazzd.db > reharm.txt
  python3 -I scripts/analysis/wjazzd_reharm.py wjazzd.db --json reharm.json
  python3 -I scripts/analysis/wjazzd_reharm.py --selftest           # no database
"""

import argparse
import collections
import json
import os
import sqlite3
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wjazzd_phrase_endings as E  # noqa: E402

SCALES = {
    "dom": {0, 2, 4, 5, 7, 9, 10},
    "min": {0, 2, 3, 5, 7, 9, 10},
    "maj": {0, 2, 4, 5, 7, 9, 11},
    "half": {0, 1, 3, 5, 6, 8, 10},
    "dim": {0, 2, 3, 5, 6, 8, 9, 11},
    "aug": {0, 2, 4, 6, 8, 10},
}
DOM_CLASS = {0: "plain", 4: "plain", 7: "plain", 10: "plain", 2: "natural", 9: "natural", 1: "altered", 3: "altered", 6: "altered", 8: "altered", 5: "other", 11: "other"}
CLASSES = ["plain", "natural", "altered", "other"]
DOM7 = {0, 4, 7, 10}
SUB = {6, 10, 1, 4}  # the tritone sub's R 3 5 b7, from the written root
TRIAD_1235 = {0, 2, 4, 7}


def tones(c):
    """A chord's R 3 5 7 as pitch classes from its root."""
    return set(c[2])


# --- spans ----------------------------------------------------------------------------------


class Solo:
    def __init__(self, notes, beats, phrases):
        self.notes = notes
        self.g = E.Beats(beats)
        self.phrases = phrases
        self.chord = []  # each note's chord (root, family, tones) or None, and its beat index
        self.beat = []
        for n in notes:
            i = self.g.index(n[1])
            self.beat.append(i)
            self.chord.append(self.g.chord[i] if i >= 0 else None)

    def spans(self):
        """The chord spans: (first beat, beats long, chord) for each run of one chord."""
        out, start, cur = [], 0, None
        for i, c in enumerate(self.g.chord):
            if self.g.change[i] or c != cur:
                if cur is not None:
                    out.append((start, i - start, cur))
                start, cur = i, c
        if cur is not None:
            out.append((start, len(self.g.chord) - start, cur))
        return out

    def notes_in(self, b0, b1):
        """Indexes of notes starting in beats b0 .. b1 (exclusive)."""
        return [k for k, i in enumerate(self.beat) if b0 <= i < b1]

    def pos(self, k):
        """Note k's place in its beat (0 .. 1)."""
        i = self.beat[k]
        if i < 0:
            return 0.0
        bl = self.g.beat_len(i)
        return (self.notes[k][1] - self.g.on[i]) / bl if bl > 0 else 0.0


def pc(note, root):
    return (note - root) % 12


# --- the measurements -------------------------------------------------------------------


def tally(c, s):
    g = s.g
    # 1 and 6: degree classes and outside notes.
    for k, n in enumerate(s.notes):
        ch = s.chord[k]
        if ch is None:
            continue
        root, fam, _ = ch
        x = pc(n[3], root)
        strong = g.pos[s.beat[k]][1] in (1, 3) and s.pos(k) < 0.1
        if fam == "dom":
            c["dom_notes"] += 1
            c["dom_" + DOM_CLASS[x]] += 1
            if strong:
                c["dom_strong"] += 1
                c["dom_strong_" + DOM_CLASS[x]] += 1
        elif fam in ("min", "maj"):
            c[fam + "_notes"] += 1
            c[fam + "_outside"] += x not in SCALES[fam]
            if fam == "maj":
                c["maj_sharp11"] += x == 6
    for b0, n_beats, ch in s.spans():
        root, fam, _ = ch
        idx = s.notes_in(b0, b0 + n_beats)
        if fam == "dom" and len(idx) >= 4:
            # 1b: the tritone sub against the written chord.
            pcs = [pc(s.notes[k][3], root) for k in idx]
            c["dom_spans"] += 1
            c["dom_sub_better"] += sum(x in SUB for x in pcs) > sum(x in DOM7 for x in pcs)
            for shift in (2, 3, 5):
                moved = [(x + shift) % 12 for x in pcs]
                c["dom_sub_chance_n"] += 1
                c["dom_sub_chance"] += sum(x in SUB for x in moved) > sum(x in DOM7 for x in moved)
        # 3: a ii-V inside a span of a bar or more.
        if n_beats >= 4 and len(idx) >= 6:
            half = b0 + n_beats / 2
            first = [k for k in idx if s.beat[k] < half]
            second = [k for k in idx if s.beat[k] >= half]
            if len(first) >= 3 and len(second) >= 3:
                if fam == "dom":
                    v_root = root
                else:
                    nxt = g.chord[b0 + n_beats] if b0 + n_beats < len(g.chord) else None
                    if nxt is None:
                        continue
                    v_root = (nxt[0] + 7) % 12
                ii = {(v_root + 7 + d) % 12 for d in (0, 3, 7, 10)}
                v = {(v_root + d) % 12 for d in DOM7}

                def fits(ks, tones_):
                    return sum(s.notes[k][3] % 12 in tones_ for k in ks) / len(ks) >= 0.6

                key = "iiv_dom" if fam == "dom" else "iiv_other"
                c[key + "_spans"] += 1
                c[key] += fits(first, ii) and fits(second, v)
                c[key + "_chance"] += fits(first, v) and fits(second, ii)
        # 5: multi-tonic runs over static harmony.
        if n_beats >= 4 and len(idx) >= 4:
            c["static_spans"] += 1
            windows = [{pc(s.notes[k][3], root) for k in idx[j : j + 4]} for j in range(len(idx) - 3)]
            windows = [w for w in windows if len(w) >= 3]

            def on(o):
                return any(all((x - o) % 12 in TRIAD_1235 for x in w) for w in windows)

            c["static_m3rd"] += on(4) or on(8)
            # Chance: the same test on the other roots (not the chord's own, its 5th or a major
            # 3rd away), two at a time, averaged.
            others = [(1, 11), (2, 10), (3, 9), (5, 6)]
            c["static_m3rd_chance"] += sum(on(a) or on(b) for a, b in others) / len(others)
    # 2: anticipation, at each change.
    for i in range(1, len(g.on)):
        if not g.change[i] or g.chord[i] is None or g.chord[i - 1] is None:
            continue
        cur, nxt = g.chord[i - 1], g.chord[i]
        cur_t = {(cur[0] + d) % 12 for d in tones(cur)}
        nxt_t = {(nxt[0] + d) % 12 for d in tones(nxt)}
        if cur_t == nxt_t:
            continue
        for k in s.notes_in(i - 1, i):
            x = s.notes[k][3] % 12
            c["ant_beat"] += 1
            c["ant_beat_fit"] += x in nxt_t and x not in cur_t
            if s.pos(k) >= 0.5:
                c["ant_half"] += 1
                c["ant_half_fit"] += x in nxt_t and x not in cur_t
        # Chance: notes two to three beats before the change (still under the current chord).
        if i >= 3 and g.chord[i - 3] == cur and not g.change[i - 1] and not g.change[i - 2]:
            for k in s.notes_in(i - 3, i - 2):
                x = s.notes[k][3] % 12
                c["ant_chance"] += 1
                c["ant_chance_fit"] += x in nxt_t and x not in cur_t
    # 4: side-slipping runs, per bar.
    bars = collections.defaultdict(lambda: [False, False])
    for pa, pz in s.phrases:
        for j in range(pa, pz - 2):
            ks = list(range(j, j + 4))
            chs = {s.chord[k][:2] if s.chord[k] else None for k in ks}
            if len(chs) != 1 or None in chs:
                continue
            root, fam, _ = s.chord[j]
            scale = SCALES.get(fam)
            if not scale:
                continue
            xs = [pc(s.notes[k][3], root) for k in ks]
            outside = sum(x not in scale for x in xs)
            if outside < 3:
                continue
            bar = s.notes[j][4]
            for slot, steps in ((0, (1, -1)), (1, (2, -2))):
                if any(sum(((x - st) % 12) in scale for x in xs) >= 3 for st in steps):
                    bars[bar][slot] = True
    n_bars = len({p[0] for p in g.pos})
    c["bars"] += n_bars
    c["slip_bars"] += sum(v[0] for v in bars.values())
    c["slip_chance_bars"] += sum(v[1] for v in bars.values())
    c["solos"] += 1


def pct(c, k, t):
    return round(100 * c[k] / c[t], 1) if c[t] else None


def summary(c):
    return dict(
        solos=c["solos"],
        dominant=dict(notes=c["dom_notes"], classes={k: pct(c, "dom_" + k, "dom_notes") for k in CLASSES},
                      strong=c["dom_strong"], strong_classes={k: pct(c, "dom_strong_" + k, "dom_strong") for k in CLASSES},
                      chance_classes={k: round(100 * sum(1 for v in DOM_CLASS.values() if v == k) / 12, 1) for k in CLASSES},
                      spans=c["dom_spans"], tritone_sub_fits_better=pct(c, "dom_sub_better", "dom_spans"), tritone_chance=pct(c, "dom_sub_chance", "dom_sub_chance_n")),
        anticipation=dict(last_beat=pct(c, "ant_beat_fit", "ant_beat"), last_beat_n=c["ant_beat"], last_half=pct(c, "ant_half_fit", "ant_half"), last_half_n=c["ant_half"],
                          chance=pct(c, "ant_chance_fit", "ant_chance"), chance_n=c["ant_chance"]),
        ii_v=dict(over_dominant=pct(c, "iiv_dom", "iiv_dom_spans"), over_dominant_chance=pct(c, "iiv_dom_chance", "iiv_dom_spans"), dominant_spans=c["iiv_dom_spans"],
                  into_next=pct(c, "iiv_other", "iiv_other_spans"), into_next_chance=pct(c, "iiv_other_chance", "iiv_other_spans"), other_spans=c["iiv_other_spans"]),
        side_slip=dict(bars=pct(c, "slip_bars", "bars"), chance=pct(c, "slip_chance_bars", "bars"), n_bars=c["bars"]),
        multi_tonic=dict(static_bars=pct(c, "static_m3rd", "static_spans"), chance=pct(c, "static_m3rd_chance", "static_spans"), n_spans=c["static_spans"]),
        minor=dict(notes=c["min_notes"], outside=pct(c, "min_outside", "min_notes"), chance=round(100 * 5 / 12, 1)),
        major=dict(notes=c["maj_notes"], outside=pct(c, "maj_outside", "maj_notes"), sharp11=pct(c, "maj_sharp11", "maj_notes"), chance=round(100 * 5 / 12, 1)),
    )


def analyse(db):
    rows = E.solos(db)
    cache = {}
    res = {}
    for slug, perf, y0, y1, feel in [p for p in E.PLAYERS if p[0] not in ("funk", "latin")]:
        c = collections.Counter()
        for melid in E.pick(rows, perf, y0, y1, feel):
            if melid not in cache:
                notes, beats, secs = E.load(db, melid)
                ok = notes and len(beats) >= 8 and all(n[3] is not None for n in notes)
                cache[melid] = Solo(notes, beats, phrases(notes, secs)) if ok else None
            if cache[melid]:
                tally(c, cache[melid])
        res[slug] = summary(c)
    return res


def phrases(notes, secs):
    if not secs:
        return []
    if max(e for _, e in secs) < len(notes):
        return [(s, e) for s, e in secs if 0 <= s <= e < len(notes)]
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [(ids[s], ids[e]) for s, e in secs if s in ids and e in ids]


# --- the report -----------------------------------------------------------------------------


def fmt(v):
    return "-" if v is None else "%g" % v


def report(res):
    L = [
        "Implied reharmonisation in the Weimar Jazz Database v2.1, by player (% unless noted; each with its",
        "chance baseline). Strong: on beats 1 and 3. Chord scales: Mixolydian, Dorian, Ionian, Locrian, whole-half,",
        "whole-tone.",
        "",
    ]
    for slug, s in res.items():
        d, a, iv, sl, mt, mi, ma = s["dominant"], s["anticipation"], s["ii_v"], s["side_slip"], s["multi_tonic"], s["minor"], s["major"]
        L.append(f"{slug}: {s['solos']} solos")
        L.append(f"  over dom7, all notes (n={d['notes']})      " + "  ".join(f"{k} {fmt(d['classes'][k])}" for k in CLASSES) + "   (chance " + " ".join(f"{fmt(v)}" for v in d["chance_classes"].values()) + ")")
        L.append(f"  over dom7, strong beats (n={d['strong']})  " + "  ".join(f"{k} {fmt(d['strong_classes'][k])}" for k in CLASSES))
        L.append(f"  dom spans fitting the tritone sub better  {fmt(d['tritone_sub_fits_better'])} of {d['spans']} (chance {fmt(d['tritone_chance'])})")
        L.append(f"  next chord anticipated: last beat {fmt(a['last_beat'])} (n={a['last_beat_n']}), last half beat {fmt(a['last_half'])} (n={a['last_half_n']}); chance {fmt(a['chance'])} (n={a['chance_n']})")
        L.append(f"  ii-V superimposed: over a long dom7 {fmt(iv['over_dominant'])} of {iv['dominant_spans']} (chance {fmt(iv['over_dominant_chance'])}); into the next chord {fmt(iv['into_next'])} of {iv['other_spans']} (chance {fmt(iv['into_next_chance'])})")
        L.append(f"  bars with a side-slipped run              {fmt(sl['bars'])} of {sl['n_bars']} (chance {fmt(sl['chance'])})")
        L.append(f"  static bars with M3rd-apart triad runs    {fmt(mt['static_bars'])} of {mt['n_spans']} spans (chance, other roots {fmt(mt['chance'])})")
        L.append(f"  outside the scale: minor 7 {fmt(mi['outside'])} (n={mi['notes']}), major {fmt(ma['outside'])} (n={ma['notes']}), #11 over major {fmt(ma['sharp11'])}; chance outside {fmt(mi['chance'])}, #11 8.3")
        L.append("")
    return "\n".join(L)


# --- a self-test on a made-up solo ------------------------------------------------------------


def selftest():
    assert DOM_CLASS[1] == "altered" and DOM_CLASS[9] == "natural" and DOM_CLASS[10] == "plain"
    db = sqlite3.connect(":memory:")
    db.executescript(
        """
        CREATE TABLE solo_info (melid, trackid, recordid, performer, signature, rhythmfeel, avgtempo);
        CREATE TABLE track_info (trackid, recordingdate);
        CREATE TABLE melody (eventid, melid, onset, duration, pitch, bar, beat, tatum, division);
        CREATE TABLE beats (melid, onset, bar, beat, chord, form);
        CREATE TABLE sections (melid, type, start, end, value);
        INSERT INTO solo_info VALUES (1, 1, 1, 'Miles Davis', '4/4', 'SWING', 120);
        INSERT INTO track_info VALUES (1, '1956-10-26');
        """
    )
    # Bar 1 G7, bar 2 C7 (a bar each), bar 3 Fmaj7, at 120 bpm (0.5 s a beat).
    for b in range(12):
        bar, beat = b // 4 + 1, b % 4 + 1
        chord = {0: "G7", 4: "C7", 8: "Fj7"}.get(b, "")
        db.execute("INSERT INTO beats VALUES (1, ?, ?, ?, ?, ?)", (b * 0.5, bar, beat, chord, "A1" if b == 0 else ""))
    # Bar 1 over G7: the tritone sub's notes (Db F Ab Cb), a quarter each. Bar 2 over C7: the ii
    # a 5th above (G Bb D F) in 8ths, then C7's own C E G, and A (Fmaj7's 3rd) on the "and" of 4,
    # anticipating it. Bar 3: F major's tones.
    notes = [(0.0, 61), (0.5, 65), (1.0, 68), (1.5, 71)]
    notes += [(2.0, 67), (2.25, 70), (2.5, 74), (2.75, 65), (3.0, 72), (3.25, 76), (3.5, 67), (3.75, 69)]
    notes += [(4.0, 65), (4.5, 69), (5.0, 72), (5.5, 76)]
    for i, (on, p) in enumerate(notes):
        b = int(on / 0.5)
        frac = (on / 0.5) - b
        db.execute("INSERT INTO melody VALUES (?, 1, ?, 0.2, ?, ?, ?, ?, 2)", (100 + i, on, p, b // 4 + 1, b % 4 + 1, 2 if frac >= 0.5 else 1))
    db.execute("INSERT INTO sections VALUES (1, 'PHRASE', 0, 15, '')")
    r = analyse(db)["miles"]
    d = r["dominant"]
    assert d["spans"] == 2 and d["tritone_sub_fits_better"] == 50.0, d  # G7's bar: all the sub's
    assert r["ii_v"]["dominant_spans"] == 1 and r["ii_v"]["over_dominant"] == 100.0, r["ii_v"]  # C7's bar (G7's has 4 notes)
    assert r["anticipation"]["last_half"] == 100.0 and r["anticipation"]["last_beat"] == 33.3, r["anticipation"]  # A; not G, nor G7's last B
    print("selftest: ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("db", nargs="?", help="wjazzd.db")
    ap.add_argument("--json", help="also write the numbers here")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if not a.db:
        ap.error("the database path (or --selftest)")
    db = sqlite3.connect(f"file:{a.db}?mode=ro", uri=True)
    res = analyse(db)
    print(report(res))
    if a.json:
        with open(a.json, "w") as f:
            json.dump(res, f, indent=1)


if __name__ == "__main__":
    main()
