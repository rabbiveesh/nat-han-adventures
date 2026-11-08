#!/usr/bin/env python3
"""Where our soloists start their phrases: the slot, the lead-in and the first note.

The partner of `wjazzd_phrase_endings.py` (same database, same players, same chord reading;
it imports that script's helpers). For the first note of every transcribed phrase:

  * the slot it starts on (1, the "and" of 1, 2 ... or elsewhere) and its bar in the 4-bar group;
  * the lead-in: how many notes come before the phrase's first note on beat 1 or 3 (the
    arrival), and which beat, bar and chord degree that arrival is;
  * the chord degree it starts on, and whether a chromatic first note resolves by a half step;
  * the first interval, the rest before the phrase, the leap from the last phrase's last note,
    and how high the start sits against the solo.

Statistics only: nothing here copies a phrase. Standard library only (python3 -I is fine):

  python3 -I scripts/analysis/wjazzd_phrase_start_slots.py wjazzd.db > slots.txt
  python3 -I scripts/analysis/wjazzd_phrase_start_slots.py wjazzd.db --json slots.json
  python3 -I scripts/analysis/wjazzd_phrase_start_slots.py --selftest           # no database
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
from wjazzd_phrase_endings import DEGREES, PLAYERS, Beats, degree, fold_degree, load, need, pick, place, shares, solos, table  # noqa: E402

SLOTS = ["1", "1&", "2", "2&", "3", "3&", "4", "4&", "other"]
LEAD_INS = ["0", "1", "2", "3", "4-7", "8+", "no arrival"]
FIRST_HOWS = ["chord tone", "tension", "chromatic, resolved by a half step", "chromatic, not resolved"]
INTERVALS = ["repeat", "step up", "step down", "3rd up", "3rd down", "4th-5th up", "4th-5th down", "wider up", "wider down"]
RESTS = ["under 1", "1-2", "2-4", "4-8", "8+"]
LEAPS = ["same", "step", "3rd", "4th-5th", "6th-octave", "wider"]


def phrase_spans(notes, secs):
    """(first, last) note index of each phrase. Sections count notes from 0 within the solo; an
    older dump counts melody eventids, so both are tried."""
    if not secs:
        return []
    if max(e for _, e in secs) < len(notes):
        return list(secs)
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [(ids[s], ids[e]) for s, e in secs if s in ids and e in ids]


def interval_class(step):
    size = abs(step)
    if size == 0:
        return "repeat"
    way = "up" if step > 0 else "down"
    if size <= 2:
        return f"step {way}"
    if size <= 4:
        return f"3rd {way}"
    if size <= 7:
        return f"4th-5th {way}"
    return f"wider {way}"


def leap_class(step):
    size = abs(step)
    if size == 0:
        return "same"
    if size <= 2:
        return "step"
    if size <= 4:
        return "3rd"
    if size <= 7:
        return "4th-5th"
    if size <= 12:
        return "6th-octave"
    return "wider"


def rest_class(beats):
    if beats < 1:
        return "under 1"
    if beats < 2:
        return "1-2"
    if beats < 4:
        return "2-4"
    if beats < 8:
        return "4-8"
    return "8+"


def lead_in_class(n):
    if n is None:
        return "no arrival"
    if n <= 3:
        return str(n)
    return "4-7" if n <= 7 else "8+"


def starting(notes, s, e, prev_e, grid, centre):
    """What the phrase from note s to note e starts on; None with no chord at its first note."""
    _, onset, _, pitch, bar, beat, tatum, division = notes[s]
    i = grid.index(onset)
    if i < 0 or grid.chord[i] is None:
        return None
    where = place(tatum, division)
    deg, kind = degree(pitch, grid.chord[i])
    how = {"ct": "chord tone", "tension": "tension"}.get(kind)
    if kind == "chromatic":
        next_up = s < e and abs(notes[s + 1][3] - pitch) == 1
        i2 = grid.index(notes[s + 1][1]) if s < e else -1
        lands = next_up and i2 >= 0 and grid.chord[i2] and degree(notes[s + 1][3], grid.chord[i2])[1] == "ct"
        how = "chromatic, resolved by a half step" if lands else "chromatic, not resolved"
    r = {
        "slot": str(beat) if where == "on" else f"{beat}&" if where == "and" else "other",
        "place": where,
        "bar_in_group": grid.bar_in_group(bar),
        "on_change": grid.change[i] and where == "on",
        "before_change": where != "on" and i + 1 < len(grid.on) and grid.change[i + 1],
        "degree": deg,
        "kind": kind,
        "how": how,
        "interval": interval_class(notes[s + 1][3] - pitch) if s < e else None,
        "above_centre": pitch - centre,
    }
    # The arrival: the phrase's first note on 1 or 3 of a 4/4 bar.
    arrival = next((k for k in range(s, e + 1) if notes[k][5] in (1, 3) and place(notes[k][6], notes[k][7]) == "on"), None)
    r["lead_in"] = None if arrival is None else arrival - s
    if arrival is not None:
        a = notes[arrival]
        a_i = grid.index(a[1])
        r["arrival_beat"] = a[5]
        r["arrival_bar"] = grid.bar_in_group(a[4])
        if a_i >= 0 and grid.chord[a_i]:
            r["arrival_degree"], r["arrival_kind"] = degree(a[3], grid.chord[a_i])
    if prev_e is not None:
        p = notes[prev_e]
        blen = grid.beat_len(i)
        r["rest_beats"] = max(0.0, onset - p[1] - (p[2] or 0)) / blen if blen > 0 else 0.0
        r["leap"] = leap_class(pitch - p[3])
    return r


def summarise(rows):
    arrivals = [r for r in rows if r["lead_in"] is not None]
    arrival_degrees = [r for r in arrivals if "arrival_degree" in r]
    after = [r for r in rows if "rest_beats" in r]
    n = max(1, len(rows))
    return {
        "phrases": len(rows),
        "slot": shares([r["slot"] for r in rows], SLOTS),
        "place": shares([r["place"] for r in rows], ["on", "and", "other"]),
        "bar_in_group": shares([r["bar_in_group"] for r in rows], [1, 2, 3, 4]),
        "on_change": round(100 * sum(bool(r["on_change"]) for r in rows) / n, 1),
        "just_before_change": round(100 * sum(bool(r["before_change"]) for r in rows) / n, 1),
        "degree": shares([r["degree"] for r in rows], DEGREES, fold_degree),
        "kind": shares([r["kind"] for r in rows], ["ct", "tension", "chromatic"]),
        "how": shares([r["how"] for r in rows], FIRST_HOWS),
        "lead_in": shares([lead_in_class(r["lead_in"]) for r in rows], LEAD_INS),
        "lead_in_median": statistics.median(r["lead_in"] for r in arrivals) if arrivals else 0,
        "arrival_beat": shares([r["arrival_beat"] for r in arrivals], [1, 3]),
        "arrival_bar": shares([r["arrival_bar"] for r in arrivals], [1, 2, 3, 4]),
        "arrival_degree": shares([r["arrival_degree"] for r in arrival_degrees], DEGREES, fold_degree),
        "arrival_kind": shares([r["arrival_kind"] for r in arrival_degrees], ["ct", "tension", "chromatic"]),
        "first_interval": shares([r["interval"] for r in rows if r["interval"]], INTERVALS),
        "rest_before": shares([rest_class(r["rest_beats"]) for r in after], RESTS),
        "rest_median": round(statistics.median(r["rest_beats"] for r in after), 2) if after else 0,
        "leap_from_last_phrase": shares([r["leap"] for r in after], LEAPS),
        "above_solo_median": round(statistics.median(r["above_centre"] for r in rows), 1) if rows else 0,
    }


def report(results):
    out = [
        "Phrase starts in the Weimar Jazz Database v2.1: the first note of each phrase, by player",
        "(% of phrases). Lead-in: notes before the phrase's first note on beat 1 or 3 (the arrival).",
        "Degrees against the chord at that note. Rest: beats since the last phrase's last note ended.",
        "",
    ]
    for slug, s in results.items():
        out.append(f"{slug}: {s['phrases']} phrases from {s['solos']} solos ({s['no_chord']} more dropped: no chord there)")
        if not s["phrases"]:
            out.append("")
            continue
        out.append(table("start slot", s["slot"]))
        out.append(table("where in the beat", s["place"]))
        out.append(table("start bar (of 4)", s["bar_in_group"]))
        out.append(f"  {'on a chord change':<26}{s['on_change']}")
        out.append(f"  {'off-beat, just before one':<26}{s['just_before_change']}")
        out.append(table("first degree", s["degree"]))
        out.append(table("first note kind", s["kind"]))
        out.append(table("how the first note sits", s["how"]))
        out.append(table("lead-in notes", s["lead_in"]))
        out.append(f"  {'lead-in median':<26}{s['lead_in_median']}")
        out.append(table("arrival beat", s["arrival_beat"]))
        out.append(table("arrival bar (of 4)", s["arrival_bar"]))
        out.append(table("arrival degree", s["arrival_degree"]))
        out.append(table("arrival kind", s["arrival_kind"]))
        out.append(table("first interval", s["first_interval"]))
        out.append(table("rest before (beats)", s["rest_before"]))
        out.append(f"  {'rest median (beats)':<26}{s['rest_median']}")
        out.append(table("from the last phrase", s["leap_from_last_phrase"]))
        out.append(f"  {'start vs solo median':<26}{s['above_solo_median']:+} semitones")
        out.append("")
    return "\n".join(out)


def analyse(db):
    need(db, "melody", ["eventid", "melid", "onset", "duration", "pitch", "bar", "beat", "tatum", "division"])
    need(db, "beats", ["melid", "onset", "bar", "beat", "chord", "form"])
    need(db, "sections", ["melid", "type", "start", "end"])
    rows = solos(db)
    cache = {}
    results = {}
    for slug, perf, y0, y1, feel in PLAYERS:
        ids = pick(rows, perf, y0, y1, feel)
        got = []
        total = 0
        for melid in ids:
            if melid not in cache:
                notes, beats, secs = load(db, melid)
                found = []
                count = 0
                if notes and len(beats) > 1:
                    grid = Beats(beats)
                    centre = statistics.median(n[3] for n in notes)
                    spans = phrase_spans(notes, secs)
                    for j, (s, e) in enumerate(spans):
                        count += 1
                        r = starting(notes, s, e, spans[j - 1][1] if j else None, grid, centre)
                        if r:
                            found.append(r)
                cache[melid] = found, count
            got.extend(cache[melid][0])
            total += cache[melid][1]
        s = summarise(got)
        s["solos"] = len(ids)
        s["no_chord"] = total - len(got)
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
    # Two bars of F7 then two of Bb7, 120 bpm (a beat is 0.5 s).
    for b in range(16):
        bar, beat = b // 4 + 1, b % 4 + 1
        chord = "F7" if b == 0 else "Bb7" if b == 8 else ""
        db.execute("INSERT INTO beats VALUES (1, ?, ?, ?, ?, ?)", (b * 0.5, bar, beat, chord, "A1" if b == 0 else ""))
    notes = [
        # On beat 1 on F7's root, up a step: no lead-in.
        (0.0, 0.25, 65, 1, 1, 1, 2), (0.5, 0.25, 67, 1, 2, 1, 2),
        # On the "and" of 2 on A (F7's 3rd), one note before beat 3.
        (2.75, 0.25, 69, 2, 2, 2, 2), (3.0, 0.25, 70, 2, 3, 1, 2),
        # On the "and" of 4 in bar 2, just before the change: F# (F7's b9), down to F on beat 1.
        (3.75, 0.25, 66, 2, 4, 2, 2), (4.0, 0.25, 65, 3, 1, 1, 2),
        # On beat 2 of bar 3, E against Bb7 (#11), no 1 or 3 after it: no arrival.
        (4.5, 0.25, 64, 3, 2, 1, 2),
    ]
    for i, (on, du, p, bar, beat, tat, div) in enumerate(notes):
        db.execute("INSERT INTO melody VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?)", (100 + i, on, du, p, bar, beat, tat, div))
    for s, e in ((0, 1), (2, 3), (4, 5), (6, 6)):
        db.execute("INSERT INTO sections VALUES (1, 'PHRASE', ?, ?, '')", (s, e))
    r = analyse(db)["miles"]
    assert r["phrases"] == 4, r
    assert r["slot"]["1"] == 25.0 and r["slot"]["2&"] == 25.0 and r["slot"]["4&"] == 25.0 and r["slot"]["2"] == 25.0, r["slot"]
    assert r["lead_in"]["no arrival"] == 25.0, r["lead_in"]
    assert r["lead_in"]["0"] == 25.0 and r["lead_in"]["1"] == 50.0, r["lead_in"]
    assert r["arrival_beat"]["1"] == 66.7 and r["arrival_beat"]["3"] == 33.3, r["arrival_beat"]
    assert r["just_before_change"] == 25.0, r["just_before_change"]
    assert r["degree"]["R"] == 25.0 and r["degree"]["3"] == 25.0, r["degree"]
    assert r["first_interval"]["step up"] == 66.7 and r["first_interval"]["step down"] == 33.3, r["first_interval"]
    assert interval_class(-7) == "4th-5th down" and leap_class(12) == "6th-octave" and rest_class(2.0) == "2-4"
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
    results = analyse(db)
    print(report(results))
    if a.json:
        with open(a.json, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    main()
