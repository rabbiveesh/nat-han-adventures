#!/usr/bin/env python3
"""Where our soloists start their phrases: the beat, the pickup, the first note.

The partner of `wjazzd_phrase_endings.py` (and built on it): for every transcribed phrase of
each of the band's measured soloists in the Weimar Jazz Database v2.1, the first note:

  * where it starts: the beat (1-4), on the beat, on the "and", or elsewhere (a triplet, a
    16th); which bar of a 4-bar group; how long the rest before it was;
  * pickups: a phrase starting in the last two beats of a bar, leading into the next bar
    line, and how many notes come before that bar line;
  * the arrival: the phrase's first note on a beat, and its first note on beat 1 or 3, with
    the beat it falls on;
  * degrees: the first note's and the arrival's, against the chord there; a start just before
    a chord change read against the coming chord (does the pickup belong to it?);
  * shape: the first move up, down or repeated; the first note above or below the phrase's
    middle (starting high and coming down?).

Statistics only: nothing here copies a phrase. Standard library only:

  python3 -I scripts/analysis/wjazzd_phrase_starts.py wjazzd.db > starts.txt
  python3 -I scripts/analysis/wjazzd_phrase_starts.py wjazzd.db --json starts.json
  python3 -I scripts/analysis/wjazzd_phrase_starts.py --selftest           # no database
"""

import argparse
import json
import os
import sqlite3
import statistics
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wjazzd_phrase_endings as E  # noqa: E402

DEGS = ["R", "3", "5", "7", "9", "11", "13", "other"]
FOLD = {"R": "R", "3": "3", "5": "5", "7": "7", "6": "13", "9": "9", "b9": "9", "#9": "9", "11": "11", "#11": "11", "13": "13", "b13": "13"}


def fold(d):
    return FOLD.get(d, "other")


def phrases(notes, secs):
    """(first, last) note indexes of each phrase (sections count notes from 0, or eventids)."""
    if not secs:
        return []
    if max(e for _, e in secs) < len(notes):
        return [(s, e) for s, e in secs if 0 <= s <= e < len(notes)]
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [(ids[s], ids[e]) for s, e in secs if s in ids and e in ids]


def start(notes, a, z, prev_end, grid):
    """What phrase notes[a..=z] opens with: a dict, or None with no chord there."""
    _, onset, dur, pitch, bar, beat, tatum, division = notes[a]
    i = grid.index(onset)
    if i < 0 or grid.chord[i] is None:
        return None
    where = E.place(tatum, division)
    deg, kind = E.degree(pitch, grid.chord[i])
    s = {
        "beat": beat,
        "place": where,
        "bar_in_group": grid.bar_in_group(bar),
        "degree": fold(deg),
        "kind": kind,
    }
    # The rest before it (beats), from the last phrase's last note's end.
    blen = grid.beat_len(i)
    if prev_end is not None and blen > 0:
        s["rest"] = max(0.0, (onset - prev_end) / blen)
    # A pickup: it starts in the bar's last two beats and the phrase reaches the next bar line.
    nb = beat >= 3 and not (beat == 3 and where == "on")
    later = [k for k in range(a, z + 1) if notes[k][4] > bar]
    s["pickup"] = bool(nb and later)
    s["pickup_notes"] = (later[0] - a) if s["pickup"] else 0
    # Just before a change: an off-beat start whose next beat changes chord, read against the
    # coming chord.
    if where != "on" and i + 1 < len(grid.on) and grid.change[i + 1] and grid.chord[i + 1]:
        s["before_change"] = E.degree(pitch, grid.chord[i + 1])[1]
    # The arrival: the first note on a beat, and the first on beat 1 or 3.
    for key, ok in (("first_on_beat", lambda b: True), ("first_strong", lambda b: b in (1, 3))):
        for k in range(a, z + 1):
            n = notes[k]
            if E.place(n[6], n[7]) == "on" and ok(n[5]):
                j = grid.index(n[1])
                if j >= 0 and grid.chord[j]:
                    s[key] = {"beat": n[5], "degree": fold(E.degree(n[3], grid.chord[j])[0]), "notes_in": k - a}
                break
    # Shape: the first move, and the first note against the phrase's middle.
    if z > a:
        mv = notes[a + 1][3] - pitch
        s["first_move"] = "up" if mv > 0 else "down" if mv < 0 else "same"
        mid = statistics.median(n[3] for n in notes[a : z + 1])
        s["starts"] = "high" if pitch > mid else "low" if pitch < mid else "middle"
    return s


def pct(items, keys):
    return E.shares(items, keys)


def summarise(st):
    n = len(st)
    arr = [s["first_on_beat"] for s in st if "first_on_beat" in s]
    strong = [s["first_strong"] for s in st if "first_strong" in s]
    pick = [s for s in st if s["pickup"]]
    bc = [s["before_change"] for s in st if "before_change" in s]
    rests = [s["rest"] for s in st if "rest" in s]
    q = lambda v: [round(x, 2) for x in statistics.quantiles(v, n=4)] if len(v) >= 4 else []
    return {
        "phrases": n,
        "place": pct([s["place"] for s in st], E.PLACES),
        "beat_any": pct([s["beat"] for s in st], [1, 2, 3, 4]),
        "beat_on_the_beat": pct([s["beat"] for s in st if s["place"] == "on"], [1, 2, 3, 4]),
        "beat_on_the_and": pct([s["beat"] for s in st if s["place"] == "and"], [1, 2, 3, 4]),
        "bar_in_group": pct([s["bar_in_group"] for s in st], [1, 2, 3, 4]),
        "rest_quartiles": q(rests),
        "pickup": round(100 * len(pick) / max(1, n), 1),
        "pickup_notes": pct([min(s["pickup_notes"], 5) for s in pick], [1, 2, 3, 4, 5]),
        "degree": pct([s["degree"] for s in st], DEGS),
        "kind": pct([s["kind"] for s in st], ["ct", "tension", "chromatic"]),
        "before_change": {"phrases": len(bc), **pct(bc, ["ct", "tension", "chromatic"])},
        "arrival_beat": pct([a["beat"] for a in arr], [1, 2, 3, 4]),
        "arrival_degree": pct([a["degree"] for a in arr], DEGS),
        "arrival_notes_in": pct([min(a["notes_in"], 4) for a in arr], [0, 1, 2, 3, 4]),
        "strong_beat": pct([a["beat"] for a in strong], [1, 3]),
        "strong_degree": pct([a["degree"] for a in strong], DEGS),
        "first_move": pct([s["first_move"] for s in st if "first_move" in s], ["up", "down", "same"]),
        "starts": pct([s["starts"] for s in st if "starts" in s], ["high", "middle", "low"]),
    }


def report(results):
    t = E.table
    out = [
        "Phrase starts in the Weimar Jazz Database v2.1: the first note of each phrase, by player (%",
        "of phrases). Pickup: starts in the bar's last two beats and runs over the bar line (notes:",
        "how many before it; 5 = five or more). Arrival: the phrase's first note on a beat (notes in:",
        "how many came before it). Before a change: an off-beat start against the coming chord.",
        "",
    ]
    for slug, s in results.items():
        out.append(f"{slug}: {s['phrases']} phrases from {s['solos']} solos ({s['no_chord']} more dropped: no chord there)")
        if not s["phrases"]:
            out.append("")
            continue
        out.append(t("where in the beat", s["place"]))
        out.append(t("beat (any)", s["beat_any"]))
        out.append(t("beat (on the beat)", s["beat_on_the_beat"]))
        out.append(t("beat (on the and)", s["beat_on_the_and"]))
        out.append(t("bar (of 4)", s["bar_in_group"]))
        out.append(f"  {'rest before (beats)':<26}quartiles {s['rest_quartiles']}")
        out.append(f"  {'pickup into a bar':<26}{s['pickup']}")
        out.append(t("pickup notes", s["pickup_notes"]))
        out.append(t("first note degree", s["degree"]))
        out.append(t("first note kind", s["kind"]))
        out.append(t("before a change", s["before_change"]))
        out.append(t("arrival beat", s["arrival_beat"]))
        out.append(t("arrival degree", s["arrival_degree"]))
        out.append(t("arrival notes in", s["arrival_notes_in"]))
        out.append(t("first 1 or 3: beat", s["strong_beat"]))
        out.append(t("first 1 or 3: degree", s["strong_degree"]))
        out.append(t("first move", s["first_move"]))
        out.append(t("first note vs middle", s["starts"]))
        out.append("")
    return "\n".join(out)


def analyse(db):
    E.need(db, "melody", ["eventid", "melid", "onset", "duration", "pitch", "bar", "beat", "tatum", "division"])
    E.need(db, "beats", ["melid", "onset", "bar", "beat", "chord", "form"])
    E.need(db, "sections", ["melid", "type", "start", "end"])
    rows = E.solos(db)
    cache = {}
    results = {}
    for slug, perf, y0, y1, feel in E.PLAYERS:
        ids = E.pick(rows, perf, y0, y1, feel)
        st, total = [], 0
        for melid in ids:
            if melid not in cache:
                notes, beats, secs = E.load(db, melid)
                got, n = [], 0
                if notes and len(beats) > 1:
                    grid = E.Beats(beats)
                    prev = None
                    for a, z in phrases(notes, secs):
                        n += 1
                        s = start(notes, a, z, prev, grid)
                        if s:
                            got.append(s)
                        prev = notes[z][1] + (notes[z][2] or 0)
                cache[melid] = (got, n)
            st.extend(cache[melid][0])
            total += cache[melid][1]
        s = summarise(st)
        s["solos"] = len(ids)
        s["no_chord"] = total - len(st)
        results[slug] = s
    return results


def selftest():
    db = sqlite3.connect(":memory:")
    db.executescript(
        """
        CREATE TABLE solo_info (melid, trackid, recordid, performer, signature, rhythmfeel);
        CREATE TABLE track_info (trackid, recordingdate);
        CREATE TABLE melody (eventid, melid, onset, duration, pitch, bar, beat, tatum, division);
        CREATE TABLE beats (melid, onset, bar, beat, chord, form);
        CREATE TABLE sections (melid, type, start, end, value);
        INSERT INTO solo_info VALUES (1, 1, 1, 'Miles Davis', '4/4', 'SWING');
        INSERT INTO track_info VALUES (1, '1956-10-26');
        """
    )
    for b in range(16):
        bar, beat = b // 4 + 1, b % 4 + 1
        chord = "F7" if b == 0 else "Bb7" if b == 8 else ""
        db.execute("INSERT INTO beats VALUES (1, ?, ?, ?, ?, ?)", (b * 0.5, bar, beat, chord, "A1" if b == 0 else ""))
    notes = [
        # On beat 1 on the root, moving up: no pickup.
        (0.0, 0.25, 65, 1, 1, 1, 2), (0.5, 0.25, 67, 1, 2, 1, 2), (1.0, 0.5, 69, 1, 3, 1, 2),
        # A pickup on the "and" of 4 of bar 2, D (the 3rd of the coming Bb7), down into bar 3.
        (3.75, 0.25, 74, 2, 4, 2, 2), (4.0, 0.25, 72, 3, 1, 1, 2), (4.5, 0.5, 70, 3, 2, 1, 2),
    ]
    for i, (on, du, p, bar, beat, tat, div) in enumerate(notes):
        db.execute("INSERT INTO melody VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?)", (100 + i, on, du, p, bar, beat, tat, div))
    for s, e in ((0, 2), (3, 5)):
        db.execute("INSERT INTO sections VALUES (1, 'PHRASE', ?, ?, '')", (s, e))
    r = analyse(db)["miles"]
    assert r["phrases"] == 2, r
    assert r["place"]["on"] == 50.0 and r["place"]["and"] == 50.0, r["place"]
    assert r["pickup"] == 50.0 and r["pickup_notes"]["1"] == 100.0, r
    assert r["before_change"]["phrases"] == 1 and r["before_change"]["ct"] == 100.0, r["before_change"]
    assert r["arrival_beat"]["1"] == 100.0 and r["arrival_degree"]["R"] == 50.0, r
    assert r["first_move"]["up"] == 50.0 and r["first_move"]["down"] == 50.0, r["first_move"]
    assert r["rest_quartiles"] == [] and r["starts"]["low"] == 50.0 and r["starts"]["high"] == 50.0, r
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
    results = analyse(sqlite3.connect(f"file:{a.db}?mode=ro", uri=True))
    print(report(results))
    if a.json:
        with open(a.json, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    main()
