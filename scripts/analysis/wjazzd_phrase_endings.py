#!/usr/bin/env python3
"""Where our soloists end their phrases: the target beat and the target note.

Reads the Weimar Jazz Database v2.1 (the sqlite file, `wjazzd.db`) and, for each of the band's
measured soloists (`music/players/*.player`), measures the last note of every transcribed
phrase:

  * the beat it lands on (1-4, on the beat, on the "and", or elsewhere), and where it sits in
    the bar group (bar 1-4 of a 4-bar group, counted from the form's section starts);
  * the chord degree it lands on (R 3 5 7, a tension 9 11 13, or a chromatic note).

A raw last note can hide its target two ways, so each is read twice, raw and as a target:

  * anticipation: an off-beat note held into the next beat (or across a chord change) is
    that next beat's note, read against that beat's chord;
  * chromaticism: a chromatic last note a half step from a chord tone is that chord tone,
    unresolved (and a last note reached by a half step into a chord tone is a chromatic
    approach, resolved).

Statistics only: nothing here copies a phrase. Standard library only (python3 -I is fine):

  python3 -I scripts/analysis/wjazzd_phrase_endings.py wjazzd.db > endings.txt
  python3 -I scripts/analysis/wjazzd_phrase_endings.py wjazzd.db --json endings.json
  python3 -I scripts/analysis/wjazzd_phrase_endings.py wjazzd.db --schema   # the tables
  python3 -I scripts/analysis/wjazzd_phrase_endings.py --selftest           # no database

The database's column names are checked at start; if a release renamed one, `--schema` shows
what is there.
"""

import argparse
import collections
import json
import re
import sqlite3
import statistics
import sys

# The band's soloists, as `music/players/*.player` measured them: performer and years, or a
# rhythm feel (the funk and Latin sections' soloists). `None` years: every year.
PLAYERS = [
    ("miles", "Miles Davis", 1951, 1959, None),
    ("trane", "John Coltrane", 1960, 1964, None),
    ("bird", "Charlie Parker", 1945, 1951, None),
    ("sonny", "Sonny Rollins", 1954, 1972, None),
    ("wayne", "Wayne Shorter", 1961, 1966, None),
    ("pops", "Louis Armstrong", 1925, 1928, None),
    ("funk", None, None, None, "FUNK"),
    ("latin", None, None, None, "LATIN"),
    ("all", None, None, None, None),  # every swing-ish 4/4 solo: the baseline
]
NOT_SWING = ("FUNK", "LATIN")

# --- chords -----------------------------------------------------------------------------

NOTE = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
INTERVAL = {0: "R", 1: "b9", 2: "9", 3: "#9", 4: "3", 5: "11", 6: "#11", 7: "5", 8: "b13", 9: "13", 10: "b7", 11: "7"}
# What a degree counts as when it isn't a chord tone: an available tension, by family.
TENSIONS = {
    "dom": {1, 2, 3, 6, 8, 9},
    "min": {2, 5, 9},
    "maj": {2, 6, 9},
    "half": {2, 5, 8},
    "dim": {2, 5, 8, 11},
    "aug": {2, 6},
}


def parse_chord(sym):
    """A WJazzD chord symbol (`C-7`, `Bbj7`, `F7alt`, `Eb-7b5`, `Do7`, `G7/B`, `NC`) as
    (root pitch class, family, chord tones {pitch classes from the root: role}); None for
    no chord."""
    if not sym:
        return None
    s = sym.strip()
    m = re.match(r"([A-G])([b#]?)(.*)$", s)
    if not m:
        return None
    root = (NOTE[m.group(1)] + {"b": -1, "#": 1, "": 0}[m.group(2)]) % 12
    q = m.group(3).split("/")[0]
    tones = {0: "R"}
    if q.startswith("-") or q.startswith("m") and not q.startswith("maj"):
        fam = "half" if "b5" in q or "7b5" in q else "min"
        tones[3] = "3"
        tones[6 if fam == "half" else 7] = "5"
        if "j" in q or "maj" in q:
            tones[11] = "7"
        elif "7" in q or "9" in q or "11" in q or fam == "half":
            tones[10] = "7"
        elif "6" in q:
            tones[9] = "6"
    elif q.startswith("o") or q.startswith("dim"):
        fam = "dim"
        tones.update({3: "3", 6: "5", 9: "7"})
    elif q.startswith("m7b5") or q.startswith("ø"):
        fam = "half"
        tones.update({3: "3", 6: "5", 10: "7"})
    elif q.startswith("+") or q.startswith("aug"):
        fam = "aug"
        tones.update({4: "3", 8: "5"})
        if "7" in q:
            tones[10] = "7"
    elif "sus" in q:
        fam = "dom" if "7" in q else "maj"
        tones.update({5: "3", 7: "5"})  # the 4th stands in for the 3rd
        if "7" in q:
            tones[10] = "7"
    elif q.startswith("j") or q.startswith("maj") or q.startswith("6") or q == "":
        fam = "maj"
        tones.update({4: "3", 7: "5"})
        if q.startswith("j") or q.startswith("maj"):
            tones[11] = "7"
        elif q.startswith("6"):
            tones[9] = "6"
    else:  # 7, 9, 13, 7alt, 7b9, 7#11 ...
        fam = "dom"
        tones[4] = "3"
        tones[8 if "#5" in q or "+" in q else 7] = "5"
        tones[10] = "7"
    return root, fam, tones


def degree(pitch, chord):
    """(label, kind) of a pitch against a chord: kind is `ct`, `tension` or `chromatic`."""
    root, fam, tones = chord
    i = (pitch - root) % 12
    if i in tones:
        return tones[i], "ct"
    if i in TENSIONS.get(fam, set()):
        return INTERVAL[i], "tension"
    return INTERVAL[i], "chromatic"


# --- the database -----------------------------------------------------------------------


def columns(db, table):
    return [r[1] for r in db.execute(f"PRAGMA table_info({table})")]


def need(db, table, cols):
    have = set(columns(db, table))
    missing = [c for c in cols if c not in have]
    if missing:
        sys.exit(f"{table}: no column {', '.join(missing)} (has {', '.join(sorted(have))}); try --schema")


def year_of(db):
    """melid -> recording year, from whichever table carries a date."""
    years = {}
    tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")]
    for table, key in (("track_info", "trackid"), ("record_info", "recordid")):
        if table not in tables:
            continue
        cols = columns(db, table)
        date = next((c for c in ("recordingdate", "recordingyear", "releasedate", "year") if c in cols), None)
        if not date:
            continue
        q = f"SELECT s.melid, t.{date} FROM solo_info s JOIN {table} t ON s.{key} = t.{key}"
        for melid, d in db.execute(q):
            m = re.search(r"(1[89]\d\d|20\d\d)", str(d or ""))
            if m and melid not in years:
                years[melid] = int(m.group(1))
    return years


def solos(db):
    need(db, "solo_info", ["melid", "performer", "signature", "rhythmfeel"])
    years = year_of(db)
    if not years:
        print("warning: no recording years found; the players' year ranges are ignored", file=sys.stderr)
    out = []
    for melid, perf, sig, feel in db.execute("SELECT melid, performer, signature, rhythmfeel FROM solo_info"):
        if (sig or "").strip() not in ("4/4", ""):
            continue
        out.append((melid, perf or "", years.get(melid), (feel or "").upper()))
    return out


def pick(solo_rows, perf, y0, y1, feel):
    got = []
    for melid, p, y, f in solo_rows:
        if feel:
            ok = f == feel
        else:
            ok = f not in NOT_SWING and (perf is None or p == perf)
            if ok and perf and y0 and y is not None:
                ok = y0 <= y <= y1
        if ok:
            got.append(melid)
    return got


def load(db, melid):
    notes = db.execute(
        "SELECT eventid, onset, duration, pitch, bar, beat, tatum, division FROM melody WHERE melid=? ORDER BY onset, eventid",
        (melid,),
    ).fetchall()
    beats = db.execute("SELECT onset, bar, beat, chord, form FROM beats WHERE melid=? ORDER BY onset", (melid,)).fetchall()
    secs = db.execute("SELECT start, end FROM sections WHERE melid=? AND type='PHRASE' ORDER BY start", (melid,)).fetchall()
    return notes, beats, secs


def phrase_ends(notes, secs):
    """The note index of each phrase's last note. Sections count notes from 0 within the
    solo; an older dump counts melody eventids, so both are tried."""
    if not secs:
        return []
    ends = [e for _, e in secs]
    if max(ends) < len(notes):
        return ends
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [ids[e] for e in ends if e in ids]


# --- one phrase end ---------------------------------------------------------------------


class Beats:
    """A solo's beat grid with its chords filled forward and its form's section starts."""

    def __init__(self, beats):
        self.on = [b[0] for b in beats]
        self.pos = [(b[1], b[2]) for b in beats]
        self.chord = []
        self.change = []
        cur = None
        for b in beats:
            c = parse_chord(b[3]) if b[3] else None
            changed = bool(b[3]) and (cur is None or (c and (c[0], c[1]) != (cur[0], cur[1])))
            if c:
                cur = c
            self.chord.append(cur)
            self.change.append(changed)
        self.section_bar = []  # bar each form section starts on, in order
        for b in beats:
            if b[4] and b[2] == 1 and (not self.section_bar or self.section_bar[-1] != b[1]):
                self.section_bar.append(b[1])

    def index(self, t):
        """The last beat at or before time t (-1 before the first)."""
        lo, hi = 0, len(self.on)
        while lo < hi:
            mid = (lo + hi) // 2
            if self.on[mid] <= t + 1e-4:
                lo = mid + 1
            else:
                hi = mid
        return lo - 1

    def beat_len(self, i):
        if 0 <= i < len(self.on) - 1:
            return self.on[i + 1] - self.on[i]
        return self.on[-1] - self.on[-2] if len(self.on) > 1 else 0.5

    def bar_in_group(self, bar):
        """1-4: the bar's place in a 4-bar group, counted from its form section's start
        (from bar 1 when the form isn't marked)."""
        start = 1
        for s in self.section_bar:
            if s <= bar:
                start = s
        return (bar - start) % 4 + 1


def place(tatum, division):
    """on, and (the 8th off the beat, swung or straight) or other."""
    if not division or tatum in (None, 0, 1):
        return "on"
    if (division % 2 == 0 and tatum - 1 == division // 2) or (division == 3 and tatum == 3):
        return "and"
    return "other"


def ending(notes, k, grid):
    """What the phrase ending at note k lands on: a dict of raw and target readings."""
    _, onset, dur, pitch, bar, beat, tatum, division = notes[k]
    i = grid.index(onset)
    if i < 0 or grid.chord[i] is None:
        return None
    blen = grid.beat_len(i)
    where = place(tatum, division)
    raw_chord = grid.chord[i]
    raw_deg, raw_kind = degree(pitch, raw_chord)
    e = {
        "beat": beat,
        "place": where,
        "bar_in_group": grid.bar_in_group(bar),
        "beats_long": (dur or 0) / blen if blen > 0 else 0,
        "raw": raw_deg,
        "raw_kind": raw_kind,
        "on_change": grid.change[i] and where == "on",
    }
    # The target beat: an off-beat note held into the next beat is that beat's note.
    t_i, t_beat, t_bar, how = i, beat, bar, "on the beat" if where == "on" else "off the beat"
    if where != "on" and i + 1 < len(grid.on):
        held = onset + (dur or 0) >= grid.on[i + 1] - 0.1 * blen
        into_change = grid.change[i + 1]
        if held or into_change and degree(pitch, grid.chord[i + 1])[1] == "ct" and raw_kind != "ct":
            t_i = i + 1
            t_bar, t_beat = grid.pos[t_i]
            how = "pushed into a change" if into_change else "pushed"
    e["target_beat"] = t_beat if how != "off the beat" else "off"
    e["target_bar_in_group"] = grid.bar_in_group(t_bar)
    e["target_on_change"] = grid.change[t_i] and how != "off the beat"
    e["how"] = how
    # The target note: a chromatic last note a half step off a chord tone is that tone.
    chord = grid.chord[t_i]
    deg, kind = degree(pitch, chord)
    note_how = "chord tone" if kind == "ct" else kind
    if kind == "chromatic":
        prev = notes[k - 1][3] if k > 0 else None
        up, down = degree(pitch + 1, chord), degree(pitch - 1, chord)
        options = [d for d in ((up, 1), (down, -1)) if d[0][1] == "ct"]
        if options:
            # Both sides: the way the line was heading.
            heading = 1 if prev is not None and pitch > prev else -1
            (deg, kind), side = next((o for o in options if o[1] == heading), options[0])
            note_how = "chromatic, unresolved (from below)" if side == 1 else "chromatic, unresolved (from above)"
    elif kind == "ct" and k > 0 and abs(pitch - notes[k - 1][3]) == 1 and degree(notes[k - 1][3], chord)[1] != "ct":
        note_how = "chromatic approach, resolved"
    e["target"] = deg
    e["target_kind"] = kind
    e["note_how"] = note_how
    return e


# --- the report -------------------------------------------------------------------------

BEATS = [1, 2, 3, 4, "off"]
PLACES = ["on", "and", "other"]
DEGREES = ["R", "3", "5", "7", "6", "9", "b9", "#9", "11", "#11", "13", "b13", "other"]
HOWS = ["on the beat", "pushed", "pushed into a change", "off the beat"]
NOTE_HOWS = ["chord tone", "chromatic approach, resolved", "tension", "chromatic, unresolved (from below)", "chromatic, unresolved (from above)", "chromatic"]


def shares(items, keys, fold=None):
    c = collections.Counter(fold(x) if fold else x for x in items)
    n = sum(c.values()) or 1
    return {str(k): round(100 * c.get(k, 0) / n, 1) for k in keys}


def fold_degree(d):
    return d if d in DEGREES else "other"


def summarise(ends):
    on_beat = [e for e in ends if e["place"] == "on"]
    beat_targets = [e for e in ends if e["target_beat"] != "off"]
    return {
        "phrases": len(ends),
        "raw_place": shares([e["place"] for e in ends], PLACES),
        "raw_beat_on_the_beat": shares([e["beat"] for e in on_beat], [1, 2, 3, 4]),
        "raw_beat_any": shares([e["beat"] for e in ends], [1, 2, 3, 4]),
        "how_it_lands": shares([e["how"] for e in ends], HOWS),
        "target_beat": shares([e["target_beat"] for e in ends], BEATS),
        "target_bar_in_group": shares([e["target_bar_in_group"] for e in beat_targets], [1, 2, 3, 4]),
        "target_on_change": round(100 * sum(e["target_on_change"] for e in ends) / max(1, len(ends)), 1),
        "raw_degree": shares([e["raw"] for e in ends], DEGREES, fold_degree),
        "raw_kind": shares([e["raw_kind"] for e in ends], ["ct", "tension", "chromatic"]),
        "target_degree": shares([e["target"] for e in ends], DEGREES, fold_degree),
        "target_kind": shares([e["target_kind"] for e in ends], ["ct", "tension", "chromatic"]),
        "note_how": shares([e["note_how"] for e in ends], NOTE_HOWS),
        "target_degree_by_beat": {
            str(b): shares([e["target"] for e in ends if e["target_beat"] == b], ["R", "3", "5", "7", "9", "other"], lambda d: d if d in ("R", "3", "5", "7", "9") else "other")
            for b in BEATS
        },
        "last_note_beats": {
            "median": round(statistics.median([e["beats_long"] for e in ends]), 2) if ends else 0,
            "held_2_beats_or_more": round(100 * sum(e["beats_long"] >= 2 for e in ends) / max(1, len(ends)), 1),
        },
    }


def table(title, d):
    return f"  {title:<26}" + "  ".join(f"{k} {v:>4}" for k, v in d.items())


def report(results):
    out = [
        "Phrase endings in the Weimar Jazz Database v2.1: the last note of each phrase, by player",
        "(% of phrases). Raw: the note as played. Target: an off-beat note held into the next",
        "beat (or into a chord change) read as that beat's, and a chromatic last note a half step",
        "off a chord tone read as that tone. Degrees against the chord at the (target) beat.",
        "",
    ]
    for slug, s in results.items():
        out.append(f"{slug}: {s['phrases']} phrases from {s['solos']} solos")
        if not s["phrases"]:
            out.append("")
            continue
        out.append(table("raw: where in the beat", s["raw_place"]))
        out.append(table("raw: beat (on the beat)", s["raw_beat_on_the_beat"]))
        out.append(table("how it lands", s["how_it_lands"]))
        out.append(table("target beat", s["target_beat"]))
        out.append(table("target bar (of 4)", s["target_bar_in_group"]))
        out.append(f"  {'target on a chord change':<26}{s['target_on_change']}")
        out.append(table("raw degree", s["raw_degree"]))
        out.append(table("target degree", s["target_degree"]))
        out.append(table("target kind", s["target_kind"]))
        out.append(table("how the note lands", s["note_how"]))
        for b, d in s["target_degree_by_beat"].items():
            out.append(table(f"degree on target beat {b}", d))
        ln = s["last_note_beats"]
        out.append(f"  {'last note (beats)':<26}median {ln['median']}, held 2+ beats {ln['held_2_beats_or_more']}%")
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
        ends = []
        for melid in ids:
            if melid not in cache:
                notes, beats, secs = load(db, melid)
                got = []
                if notes and len(beats) > 1:
                    grid = Beats(beats)
                    for k in phrase_ends(notes, secs):
                        e = ending(notes, k, grid)
                        if e:
                            got.append(e)
                cache[melid] = got
            ends.extend(cache[melid])
        s = summarise(ends)
        s["solos"] = len(ids)
        results[slug] = s
    return results


# --- a self-test on a made-up database --------------------------------------------------


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
        # A phrase ending on beat 3 on the 3rd of F7 (A).
        (0.0, 0.25, 65, 1, 1, 1, 2), (0.5, 0.25, 67, 1, 2, 1, 2), (1.0, 1.0, 69, 1, 3, 1, 2),
        # One pushed: the "and" of 4 in bar 2, held over the change, D (the 3rd of Bb7).
        (2.5, 0.25, 72, 2, 2, 1, 2), (3.75, 1.0, 74, 2, 4, 2, 2),
        # One ending chromatic: B over Bb7 (a half step above the root), heading up from A.
        (5.0, 0.25, 69, 3, 3, 1, 2), (5.5, 0.5, 71, 3, 4, 1, 2),
        # One unresolved: A over Bb7 (the major 7th, chromatic), heading up: the root's below.
        (6.0, 0.25, 67, 4, 1, 1, 2), (6.5, 0.5, 69, 4, 2, 1, 2),
    ]
    for i, (on, du, p, bar, beat, tat, div) in enumerate(notes):
        db.execute("INSERT INTO melody VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?)", (100 + i, on, du, p, bar, beat, tat, div))
    for s, e in ((0, 2), (3, 4), (5, 6), (7, 8)):
        db.execute("INSERT INTO sections VALUES (1, 'PHRASE', ?, ?, '')", (s, e))
    r = analyse(db)["miles"]
    assert r["phrases"] == 4, r
    assert r["how_it_lands"]["pushed into a change"] == 25.0, r["how_it_lands"]
    assert [r["target_beat"][b] for b in "1234"] == [25.0, 25.0, 25.0, 25.0], r["target_beat"]
    assert r["target_degree"]["3"] == 50.0 and r["target_degree"]["R"] == 25.0, r["target_degree"]
    assert r["raw_degree"]["b9"] == 25.0 and r["raw_degree"]["13"] == 25.0, r["raw_degree"]  # raw: B over Bb7, D over F7
    assert r["note_how"]["tension"] == 25.0, r["note_how"]  # a b9 over a dominant is a tension, not chromatic
    assert r["note_how"]["chromatic, unresolved (from below)"] == 25.0, r["note_how"]
    assert r["target_bar_in_group"]["3"] == 50.0, r["target_bar_in_group"]
    assert parse_chord("Eb-7b5")[1] == "half" and parse_chord("Bbj7")[2][11] == "7" and parse_chord("NC") is None
    assert degree(66, parse_chord("C7")) == ("#11", "tension") and degree(64, parse_chord("C-7")) == ("3", "chromatic")
    print("selftest: ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("db", nargs="?", help="wjazzd.db")
    ap.add_argument("--json", help="also write the numbers here")
    ap.add_argument("--schema", action="store_true", help="print the tables and their columns")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if not a.db:
        ap.error("the database path (or --selftest)")
    db = sqlite3.connect(f"file:{a.db}?mode=ro", uri=True)
    if a.schema:
        for (t,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"):
            print(t, columns(db, t))
        return
    results = analyse(db)
    print(report(results))
    if a.json:
        with open(a.json, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    main()
