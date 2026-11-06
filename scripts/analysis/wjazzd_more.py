#!/usr/bin/env python3
"""Our soloists' remaining habits: blue notes, tension resolution, displacement, the bass, long notes.

For each of the band's measured soloists in the Weimar Jazz Database v2.1, and the whole
database, each against a chance baseline where one applies:

  1. blue notes: the b3 and the b5 over major and dominant chords (chance: 1/12 each), and
     how often a b3 moves up a half step to the 3rd next;
  2. tension resolution: after a tension or a chromatic note on beat 1 or 3, the next note a
     step (1-2 semitones) onto a chord tone (chance: the same after a chord tone there);
  3. rhythmic displacement: runs of three or more gaps of a dotted quarter (1.5 beats, the
     3-over-4 hemiola), and 8th-note lines whose contour repeats every 3 notes, as a share of
     phrases (chance for the contour: every 4 notes, the bar's own grouping);
  4. the soloist against the bass (the database's beat-wise bass pitch): on beats 1 and 3,
     the soloist's note against the bass note by interval class: unison or octave, a 3rd or
     6th, a 5th or 4th, a 2nd or 7th, the tritone (chance: the same against the bass a beat
     later);
  5. long notes (a beat or longer): their degree class against the chord (chord tone,
     tension, chromatic), against all notes;
  6. phrase length by tempo band: notes and beats per phrase, and the rest after.

Phrases are the database's hand-marked ones. Statistics only: nothing here copies a phrase.
Standard library only:

  python3 -I scripts/analysis/wjazzd_more.py wjazzd.db > more.txt
  python3 -I scripts/analysis/wjazzd_more.py wjazzd.db --json more.json
  python3 -I scripts/analysis/wjazzd_more.py --selftest           # no database
"""

import argparse
import collections
import json
import os
import sqlite3
import statistics
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wjazzd_phrase_endings as E  # noqa: E402

BANDS = [(0, 140), (140, 200), (200, 260), (260, 1000)]
BAND_NAMES = ["<140", "140-200", "200-260", ">260"]
BASS = ["unison/8ve", "3rd/6th", "5th/4th", "2nd/7th", "tritone"]


def bass_class(iv):
    iv %= 12
    return {0: 0, 3: 1, 4: 1, 8: 1, 9: 1, 5: 2, 7: 2, 1: 3, 2: 3, 10: 3, 11: 3, 6: 4}[iv]


def pct(a, b):
    return round(100 * a / b, 1) if b else None


def med(v, n=2):
    return round(statistics.median(v), n) if v else None


def spans(notes, secs):
    if not secs:
        return []
    if max(e for _, e in secs) < len(notes):
        return [(s, e) for s, e in secs if 0 <= s <= e < len(notes)]
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [(ids[s], ids[e]) for s, e in secs if s in ids and e in ids]


def tally(c, L, notes, beats, phrases, bass, tempo):
    g = E.Beats(beats)
    at = {pos: i for i, pos in enumerate(g.pos)}
    beat = [g.index(n[1]) for n in notes]
    chord = [g.chord[i] if i >= 0 else None for i in beat]
    strong = [n[5] in (1, 3) and n[6] in (None, 0, 1) for n in notes]
    c["solos"] += 1
    for k, n in enumerate(notes):
        ch = chord[k]
        if ch is None:
            continue
        root, fam, _ = ch
        x = (n[3] - root) % 12
        d, kind = E.degree(n[3], ch)
        # 1. Blue notes.
        if fam in ("maj", "dom"):
            c["blue_n"] += 1
            c["blue_b3"] += x == 3
            c["blue_b5"] += x == 6
            if x == 3 and k + 1 < len(notes):
                c["b3_next"] += 1
                c["b3_up"] += notes[k + 1][3] - n[3] == 1
        # 2. Tension resolution.
        if strong[k] and k + 1 < len(notes) and chord[k + 1] is not None:
            nxt = notes[k + 1][3]
            ok = 1 <= abs(nxt - n[3]) <= 2 and E.degree(nxt, chord[k + 1])[1] == "ct"
            key = "res_ct" if kind == "ct" else "res_tension"
            c[key + "_n"] += 1
            c[key] += ok
        # 4. Against the bass.
        if strong[k]:
            i = at.get((n[4], n[5]), -1)
            if 0 <= i < len(bass) and bass[i]:
                c["bass_n"] += 1
                c["bass_%d" % bass_class(n[3] - bass[i])] += 1
                if i + 1 < len(bass) and bass[i + 1]:
                    c["bass_chance_n"] += 1
                    c["bass_chance_%d" % bass_class(n[3] - bass[i + 1])] += 1
        # 5. Long notes.
        bi = beat[k]
        bl = g.beat_len(bi) if bi >= 0 else 0
        if bl > 0:
            c["all_n"] += 1
            c["all_" + kind] += 1
            if (n[2] or 0) >= bl:
                c["long_n"] += 1
                c["long_" + kind] += 1
    # 3. Displacement, and 6. phrase length.
    band = next((j for j, (lo, hi) in enumerate(BANDS) if lo <= (tempo or 0) < hi), None)
    for j, (pa, pz) in enumerate(phrases):
        if pz - pa < 3:
            continue
        c["phrases"] += 1
        gaps = []
        for k in range(pa, pz):
            bi = beat[k]
            bl = g.beat_len(bi) if bi >= 0 else 0
            gaps.append((notes[k + 1][1] - notes[k][1]) / bl if bl > 0 else None)
        run, best = 0, 0
        for x in gaps:
            run = run + 1 if x is not None and abs(x - 1.5) < 0.15 else 0
            best = max(best, run)
        c["hemiola"] += best >= 3
        # 8th lines with a contour repeating every 3 notes (or 4, the chance).
        eighths = [x is not None and 0.35 < x < 0.7 for x in gaps]
        signs = [(notes[k + 1][3] > notes[k][3]) - (notes[k + 1][3] < notes[k][3]) for k in range(pa, pz)]
        for period, key in ((3, "contour3"), (4, "contour4")):
            hit = False
            for s0 in range(len(signs) - 2 * period + 1):
                w = signs[s0 : s0 + 2 * period]
                if all(eighths[s0 : s0 + 2 * period]) and w[:period] == w[period:] and len(set(w[:period])) > 1:
                    hit = True
                    break
            c[key] += hit
        if band is not None:
            bl = g.beat_len(beat[pa]) if beat[pa] >= 0 else 0
            if bl > 0:
                L["len_notes_%d" % band].append(pz - pa + 1)
                L["len_beats_%d" % band].append((notes[pz][1] + (notes[pz][2] or 0) - notes[pa][1]) / bl)
                if j + 1 < len(phrases):
                    na = phrases[j + 1][0]
                    L["rest_%d" % band].append(max(0.0, notes[na][1] - notes[pz][1] - (notes[pz][2] or 0)) / bl)


def summary(c, L):
    return dict(
        solos=c["solos"],
        blue=dict(b3=pct(c["blue_b3"], c["blue_n"]), b5=pct(c["blue_b5"], c["blue_n"]), chance=8.3, b3_up_to_3=pct(c["b3_up"], c["b3_next"]), notes=c["blue_n"]),
        resolution=dict(after_tension=pct(c["res_tension"], c["res_tension_n"]), n=c["res_tension_n"], after_chord_tone=pct(c["res_ct"], c["res_ct_n"])),
        displacement=dict(hemiola=pct(c["hemiola"], c["phrases"]), contour_every_3=pct(c["contour3"], c["phrases"]), contour_every_4=pct(c["contour4"], c["phrases"]), phrases=c["phrases"]),
        bass=dict(n=c["bass_n"], classes={BASS[j]: pct(c["bass_%d" % j], c["bass_n"]) for j in range(5)}, chance={BASS[j]: pct(c["bass_chance_%d" % j], c["bass_chance_n"]) for j in range(5)}),
        long_notes=dict(n=c["long_n"], long={k: pct(c["long_" + k], c["long_n"]) for k in ("ct", "tension", "chromatic")}, all={k: pct(c["all_" + k], c["all_n"]) for k in ("ct", "tension", "chromatic")}),
        phrase_length={BAND_NAMES[b]: dict(notes=med(L["len_notes_%d" % b], 1), beats=med(L["len_beats_%d" % b], 1), rest=med(L["rest_%d" % b], 1), phrases=len(L["len_notes_%d" % b])) for b in range(4)},
    )


def analyse(db):
    rows = E.solos(db)
    tempo = dict(db.execute("SELECT melid, avgtempo FROM solo_info"))
    cache = {}
    res = {}
    for slug, perf, y0, y1, feel in [p for p in E.PLAYERS if p[0] not in ("funk", "latin")]:
        c, L = collections.Counter(), collections.defaultdict(list)
        for melid in E.pick(rows, perf, y0, y1, feel):
            if melid not in cache:
                notes, beats, secs = E.load(db, melid)
                ok = notes and len(beats) >= 8 and all(n[3] is not None for n in notes)
                bass = [b if isinstance(b, (int, float)) and b > 0 else None for (b,) in db.execute("SELECT bass_pitch FROM beats WHERE melid=? ORDER BY onset", (melid,))] if ok else []
                cache[melid] = (notes, beats, spans(notes, secs), bass, tempo.get(melid)) if ok else None
            if cache[melid]:
                tally(c, L, *cache[melid])
        res[slug] = summary(c, L)
    return res


def fmt(v):
    return "-" if v is None else "%g" % v


def report(res):
    out = ["More habits in the Weimar Jazz Database v2.1, by player (%, each with its chance baseline where one", "applies; strong: on beats 1 and 3; phrase lengths in beats are medians).", ""]
    for slug, s in res.items():
        b, r, d, ba, ln = s["blue"], s["resolution"], s["displacement"], s["bass"], s["long_notes"]
        out.append(f"{slug}: {s['solos']} solos")
        out.append(f"  blue notes over major/dom (n={b['notes']}): b3 {fmt(b['b3'])}, b5 {fmt(b['b5'])} (chance {b['chance']} each); a b3 up to the 3rd next {fmt(b['b3_up_to_3'])}")
        out.append(f"  a strong-beat tension or chromatic note stepping onto a chord tone {fmt(r['after_tension'])} (n={r['n']}); after a chord tone {fmt(r['after_chord_tone'])}")
        out.append(f"  phrases with a dotted-quarter hemiola {fmt(d['hemiola'])}; an 8th line repeating its contour every 3 notes {fmt(d['contour_every_3'])} (every 4: {fmt(d['contour_every_4'])}) (n={d['phrases']})")
        out.append(f"  against the bass on 1 and 3 (n={ba['n']})  " + "  ".join(f"{k} {fmt(v)}" for k, v in ba["classes"].items()) + "   (chance " + " ".join(fmt(v) for v in ba["chance"].values()) + ")")
        out.append(f"  long notes (n={ln['n']}): chord tone {fmt(ln['long']['ct'])}, tension {fmt(ln['long']['tension'])}, chromatic {fmt(ln['long']['chromatic'])}  (all notes {fmt(ln['all']['ct'])}/{fmt(ln['all']['tension'])}/{fmt(ln['all']['chromatic'])})")
        out.append("  phrase length by tempo   " + "  ".join(f"{k}: {fmt(v['notes'])} notes, {fmt(v['beats'])} beats, rest {fmt(v['rest'])} (n={v['phrases']})" for k, v in s["phrase_length"].items()))
        out.append("")
    return "\n".join(out)


def selftest():
    assert bass_class(12) == 0 and bass_class(4) == 1 and bass_class(7) == 2 and bass_class(10) == 3 and bass_class(6) == 4
    db = sqlite3.connect(":memory:")
    db.executescript(
        """
        CREATE TABLE solo_info (melid, trackid, recordid, performer, signature, rhythmfeel, avgtempo);
        CREATE TABLE track_info (trackid, recordingdate);
        CREATE TABLE melody (eventid, melid, onset, duration, pitch, bar, beat, tatum, division);
        CREATE TABLE beats (melid, onset, bar, beat, chord, form, bass_pitch);
        CREATE TABLE sections (melid, type, start, end, value);
        INSERT INTO solo_info VALUES (1, 1, 1, 'Miles Davis', '4/4', 'SWING', 120);
        INSERT INTO track_info VALUES (1, '1956-10-26');
        """
    )
    # Two bars of C7 at 120 bpm, the bass on C (36) throughout.
    for b in range(8):
        db.execute("INSERT INTO beats VALUES (1, ?, ?, ?, ?, ?, 36)", (b * 0.5, b // 4 + 1, b % 4 + 1, "C7" if b == 0 else "", "A1" if b == 0 else ""))
    # Beat 1: Eb (the b3) up to E; beat 3: Db (b9) down to C; then quarters on G, E.
    for i, (on, p, beat) in enumerate(((0.0, 63, 1), (0.5, 64, 2), (1.0, 61, 3), (1.5, 60, 4), (2.0, 67, 1), (2.5, 64, 2))):
        db.execute("INSERT INTO melody VALUES (?, 1, ?, 0.45, ?, ?, ?, 1, 1)", (100 + i, on, p, 1 if on < 2 else 2, beat))
    db.execute("INSERT INTO sections VALUES (1, 'PHRASE', 0, 5, '')")
    r = analyse(db)["miles"]
    assert r["blue"]["b3"] == 16.7 and r["blue"]["b3_up_to_3"] == 100.0, r["blue"]
    assert r["resolution"]["after_tension"] == 100.0, r["resolution"]  # Eb to E, Db to C
    assert r["bass"]["n"] == 3, r["bass"]
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
