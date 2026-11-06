#!/usr/bin/env python3
"""Our soloists' style beyond the notes: timing, register, form, habits, dynamics, articulation.

For each of the band's measured soloists in the Weimar Jazz Database v2.1, and the whole
database (tempo bands: under 140 bpm, 140-200, 200-260, over 260):

  1. swing and timing by tempo band: the swing ratio (the 8th pair inside a beat, long over
     short), the on-beat lay-back (ms against the annotated beat), the off-beat 8th's place
     against the beat's midpoint (ms), and the jitter (each solo's on-beat SD, the median);
  2. register across choruses: the range used per chorus (5th to 95th percentile, semitones),
     the share of later choruses that reach a new high for the solo, and how the register's
     centre moves from chorus to chorus;
  3. rhythm against the chords: phrase starts and ends within a beat of a chord change, and
     an attack on the "and" before a change held over it, each against chance;
  4. density through the form: notes per bar by bar of an 8-bar section, and in a chorus's
     last 2 bars (the turnaround), against the solo's mean;
  5. interval and contour habits by tempo band: steps, leaps, notes in runs of 4 or more one
     way, and the share of 3rds inside those runs (arpeggiated against scalar);
  6. dynamics (the database's loudness per note, its -66 dB failed measurements left out):
     dB against the solo's median by register, by place in the phrase and by chorus;
  7. articulation by tempo band: a note's length over the time to the next one (legato), and
     the share of off-beat notes 6 dB or more under their phrase's median (ghosted; the
     database doesn't mark ghost notes, so loudness stands in).

Phrases are the database's hand-marked ones. Statistics only: nothing here copies a phrase.
Standard library only:

  python3 -I scripts/analysis/wjazzd_style.py wjazzd.db > style.txt
  python3 -I scripts/analysis/wjazzd_style.py wjazzd.db --json style.json
  python3 -I scripts/analysis/wjazzd_style.py --selftest           # no database
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
FAILED_DB = 20  # loud_med at or under this is the database's failed measurement (-66)


def band_of(tempo):
    return next((k for k, (lo, hi) in enumerate(BANDS) if lo <= (tempo or 0) < hi), None)


def med(v, n=2):
    return round(statistics.median(v), n) if v else None


def pct(a, b):
    return round(100 * a / b, 1) if b else None


def quantile(v, q):
    v = sorted(v)
    if not v:
        return None
    k = (len(v) - 1) * q
    lo = int(k)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (v[hi] - v[lo]) * (k - lo)


class Solo:
    def __init__(self, notes, beats, phrases, tempo, loud, chorus_of_beat):
        self.notes, self.phrases, self.tempo = notes, phrases, tempo or 0
        self.g = E.Beats(beats)
        self.loud = loud  # eventid -> dB (or None)
        self.beat = [self.g.index(n[1]) for n in notes]
        self.chorus_of_beat = chorus_of_beat  # beat index -> chorus id (1..), or None

    def frac(self, k):
        i = self.beat[k]
        if i < 0:
            return None
        bl = self.g.beat_len(i)
        return (self.notes[k][1] - self.g.on[i]) / bl if bl > 0 else None

    def chorus(self, k):
        i = self.beat[k]
        return self.chorus_of_beat[i] if 0 <= i < len(self.chorus_of_beat) else None


def load(db, melid, tempo):
    notes, beats, secs = E.load(db, melid)
    if not notes or len(beats) < 8 or any(n[3] is None for n in notes):
        return None
    loud = {}
    for eid, l in db.execute("SELECT eventid, loud_med FROM melody WHERE melid=?", (melid,)):
        loud[eid] = l if l is not None and l > FAILED_DB else None
    ch = [c for (c,) in db.execute("SELECT chorus_id FROM beats WHERE melid=? ORDER BY onset", (melid,))]
    ch = [c if isinstance(c, int) and c >= 1 else None for c in ch]
    return Solo(notes, beats, spans(notes, secs), tempo, loud, ch)


def spans(notes, secs):
    if not secs:
        return []
    if max(e for _, e in secs) < len(notes):
        return [(s, e) for s, e in secs if 0 <= s <= e < len(notes)]
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [(ids[s], ids[e]) for s, e in secs if s in ids and e in ids]


# --- the measurements -------------------------------------------------------------------


def tally(c, L, s):
    g, notes = s.g, s.notes
    b = band_of(s.tempo)
    bk = "b%d_" % b if b is not None else None
    c["solos"] += 1
    # 1. Swing and timing.
    on_dev = []
    at = {pos: i for i, pos in enumerate(g.pos)}
    for k, n in enumerate(notes):
        # Against the beat the transcription puts the note on (a note played early would fall
        # into the beat before by time alone).
        i = at.get((n[4], n[5]), -1)
        if i < 0 or i + 1 >= len(g.on):
            continue
        bl = g.on[i + 1] - g.on[i]
        if bl <= 0:
            continue
        x = (n[1] - g.on[i]) / bl
        if n[6] in (None, 0, 1) and abs(x) <= 0.125:
            on_dev.append(1000 * (n[1] - g.on[i]))
        if n[7] == 2 and n[6] == 2 and 0.35 < x < 0.85 and bk:
            L[bk + "swing"].append(x / (1 - x))
            L[bk + "offbeat_ms"].append(1000 * (n[1] - (g.on[i] + bl / 2)))
    if bk and len(on_dev) >= 10:
        L[bk + "lay_back"].extend(on_dev)
        L[bk + "jitter"].append(statistics.pstdev(on_dev))
    # 2. Register across choruses.
    by_ch = collections.defaultdict(list)
    for k, n in enumerate(notes):
        cid = s.chorus(k)
        if cid:
            by_ch[cid].append(n[3])
    chs = [by_ch[x] for x in sorted(by_ch) if len(by_ch[x]) >= 10]
    high = None
    prev_c = None
    for j, ps in enumerate(chs):
        L["range"].append(quantile(ps, 0.95) - quantile(ps, 0.05))
        top, centre = max(ps), statistics.median(ps)
        if j:
            c["later_choruses"] += 1
            c["new_high"] += top > high
            L["centre_move"].append(centre - prev_c)
        high = top if high is None else max(high, top)
        prev_c = centre
    # 3. Rhythm against the chords.
    change_beats = [i for i in range(len(g.on)) if g.change[i]]
    if change_beats:
        near = [0] * len(g.on)
        for i in change_beats:
            for d in (-1, 0, 1):
                if 0 <= i + d < len(g.on):
                    near[i + d] = 1
        c["chance_near"] += sum(near)
        c["chance_beats"] += len(g.on)
        for pa, pz in s.phrases:
            for k, key in ((pa, "start"), (pz, "end")):
                i = s.beat[k]
                if 0 <= i < len(g.on):
                    c[key + "s"] += 1
                    fr = s.frac(k) or 0.0
                    j = i + 1 if fr > 0.5 else i  # the nearest beat
                    c[key + "_near"] += any(abs(j - x) <= 1 for x in change_beats if abs(j - x) <= 1)
        # An attack on the "and" before beat i, held over it.
        changes = set(change_beats)
        for k, n in enumerate(notes):
            i = s.beat[k]
            if i < 0 or i + 1 >= len(g.on) or not (n[7] == 2 and n[6] == 2):
                continue
            held = n[1] + (n[2] or 0) >= g.on[i + 1] - 0.05 * g.beat_len(i)
            if g.pos[i + 1][1] == 1 or (i + 1) in changes:
                key = "ant_change" if (i + 1) in changes else "ant_bar"
                c[key + "_ands"] += 1
                c[key + "_held"] += held
        c["changes"] += len(change_beats)
    # 4. Density through the form.
    bars = collections.Counter(n[4] for n in notes)
    bar_list = sorted({p[0] for p in g.pos})
    if bar_list:
        mean = len(notes) / len(bar_list)
        if mean > 0:
            for bar in bar_list:
                start = max([x for x in g.section_bar if x <= bar], default=1)
                L["sec_bar_%d" % ((bar - start) % 8 + 1)].append(bars[bar] / mean)
            last_of = {}
            for i, cid in enumerate(s.chorus_of_beat):
                if cid:
                    last_of[cid] = max(last_of.get(cid, -10**9), g.pos[i][0])
            for cid, last in last_of.items():
                for bar in (last - 1, last):
                    L["turnaround"].append(bars[bar] / mean)
    # 5. Interval and contour habits.
    if bk:
        for pa, pz in s.phrases:
            iv = [notes[k + 1][3] - notes[k][3] for k in range(pa, pz)]
            if not iv:
                continue
            c[bk + "ivs"] += len(iv)
            c[bk + "steps"] += sum(1 <= abs(x) <= 2 for x in iv)
            c[bk + "leaps"] += sum(abs(x) >= 5 for x in iv)
            c[bk + "notes"] += len(iv) + 1
            j = 0
            while j < len(iv):
                if iv[j] == 0:
                    j += 1
                    continue
                sign = iv[j] > 0
                e = j
                while e + 1 < len(iv) and iv[e + 1] != 0 and (iv[e + 1] > 0) == sign:
                    e += 1
                if e - j + 1 >= 3:  # 3 intervals: 4 notes one way
                    c[bk + "run_notes"] += e - j + 2
                    run = iv[j : e + 1]
                    c[bk + "run_ivs"] += len(run)
                    c[bk + "run_thirds"] += sum(3 <= abs(x) <= 4 for x in run)
                j = e + 1
    # 6 and 7. Dynamics and articulation.
    louds = [s.loud.get(n[0]) for n in notes]
    have = [x for x in louds if x is not None]
    if len(have) >= 20:
        m = statistics.median(have)
        pitches = [n[3] for n in notes]
        lo, hi = quantile(pitches, 1 / 3), quantile(pitches, 2 / 3)
        for k, x in enumerate(louds):
            if x is None:
                continue
            p = pitches[k]
            L["dyn_" + ("low" if p < lo else "high" if p > hi else "mid")].append(x - m)
            cid = s.chorus(k)
            if cid:
                L["dyn_chorus_%d" % min(cid, 4)].append(x - m)
        for pa, pz in s.phrases:
            n_ = pz - pa + 1
            if n_ < 5:
                continue
            ph = [louds[k] for k in range(pa, pz + 1) if louds[k] is not None]
            if len(ph) < 4:
                continue
            pm = statistics.median(ph)
            for k in range(pa, pz + 1):
                x = louds[k]
                if x is None:
                    continue
                place = "first" if k == pa else "last" if k == pz else ("early", "middle", "late")[min(2, 3 * (k - pa) // n_)]
                L["dyn_" + place].append(x - m)
                fr = s.frac(k)
                if bk and fr is not None and 0.2 < fr < 0.9:
                    c[bk + "offbeats"] += 1
                    c[bk + "ghosts"] += x <= pm - 6
    if bk:
        for pa, pz in s.phrases:
            for k in range(pa, pz):
                ioi = notes[k + 1][1] - notes[k][1]
                if ioi > 0 and notes[k][2]:
                    L[bk + "legato"].append(min(2.0, notes[k][2] / ioi))


def summary(c, L):
    out = {"solos": c["solos"]}
    out["timing"] = {
        BAND_NAMES[b]: dict(
            swing=med(L["b%d_swing" % b]), lay_back_ms=med(L["b%d_lay_back" % b], 1), offbeat_vs_midpoint_ms=med(L["b%d_offbeat_ms" % b], 1),
            jitter_ms=med(L["b%d_jitter" % b], 1), pairs=len(L["b%d_swing" % b]), solos=len(L["b%d_jitter" % b]),
        )
        for b in range(4)
    }
    out["register"] = dict(range_median=med(L["range"], 1), choruses=len(L["range"]), new_high=pct(c["new_high"], c["later_choruses"]), later_choruses=c["later_choruses"],
                           centre_move_median_abs=med([abs(x) for x in L["centre_move"]], 1), centre_move_mean=round(statistics.mean(L["centre_move"]), 2) if L["centre_move"] else None)
    out["chords"] = dict(starts_near_change=pct(c["start_near"], c["starts"]), ends_near_change=pct(c["end_near"], c["ends"]), chance=pct(c["chance_near"], c["chance_beats"]),
                         held_and_into_change=pct(c["ant_change_held"], c["ant_change_ands"]), held_and_into_bar_without_change=pct(c["ant_bar_held"], c["ant_bar_ands"]),
                         ands_before_changes=c["ant_change_ands"], ands_before_bars=c["ant_bar_ands"])
    out["form"] = dict(section_bars=[med(L["sec_bar_%d" % k]) for k in range(1, 9)], turnaround=med(L["turnaround"]))
    out["habits"] = {
        BAND_NAMES[b]: dict(steps=pct(c["b%d_steps" % b], c["b%d_ivs" % b]), leaps=pct(c["b%d_leaps" % b], c["b%d_ivs" % b]), in_runs=pct(c["b%d_run_notes" % b], c["b%d_notes" % b]),
                            thirds_in_runs=pct(c["b%d_run_thirds" % b], c["b%d_run_ivs" % b]), intervals=c["b%d_ivs" % b])
        for b in range(4)
    }
    out["dynamics"] = dict(register={k: med(L["dyn_" + k], 1) for k in ("low", "mid", "high")},
                           phrase={k: med(L["dyn_" + k], 1) for k in ("first", "early", "middle", "late", "last")},
                           chorus={str(k) + ("+" if k == 4 else ""): med(L["dyn_chorus_%d" % k], 1) for k in range(1, 5)}, notes=sum(len(L["dyn_" + k]) for k in ("low", "mid", "high")))
    out["articulation"] = {BAND_NAMES[b]: dict(legato=med(L["b%d_legato" % b]), ghosted_offbeats=pct(c["b%d_ghosts" % b], c["b%d_offbeats" % b]), offbeats=c["b%d_offbeats" % b]) for b in range(4)}
    return out


def analyse(db):
    rows = E.solos(db)
    tempo = dict(db.execute("SELECT melid, avgtempo FROM solo_info"))
    cache = {}
    res = {}
    for slug, perf, y0, y1, feel in [p for p in E.PLAYERS if p[0] not in ("funk", "latin")]:
        c, L = collections.Counter(), collections.defaultdict(list)
        for melid in E.pick(rows, perf, y0, y1, feel):
            if melid not in cache:
                cache[melid] = load(db, melid, tempo.get(melid))
            if cache[melid]:
                tally(c, L, cache[melid])
        res[slug] = summary(c, L)
    return res


# --- the report -----------------------------------------------------------------------------


def fmt(v):
    return "-" if v is None else "%g" % v


def report(res):
    out = [
        "Style in the Weimar Jazz Database v2.1, by player (hand-marked phrases; tempo bands by the solo's",
        "average tempo; dB against the solo's median loudness; ratios against the solo's mean).",
        "",
    ]
    for slug, s in res.items():
        out.append(f"{slug}: {s['solos']} solos")
        for b, t in s["timing"].items():
            out.append(f"  timing {b:<8} swing {fmt(t['swing'])} (n={t['pairs']})  lay-back {fmt(t['lay_back_ms'])} ms  off-beat 8th vs midpoint {fmt(t['offbeat_vs_midpoint_ms'])} ms  jitter {fmt(t['jitter_ms'])} ms (solos {t['solos']})")
        r = s["register"]
        out.append(f"  register: range per chorus {fmt(r['range_median'])} st (n={r['choruses']}); a new high in {fmt(r['new_high'])}% of later choruses (n={r['later_choruses']}); centre moves {fmt(r['centre_move_median_abs'])} st a chorus (mean {fmt(r['centre_move_mean'])})")
        ch = s["chords"]
        out.append(f"  phrase starts within a beat of a change {fmt(ch['starts_near_change'])}%, ends {fmt(ch['ends_near_change'])}% (chance {fmt(ch['chance'])}%)")
        out.append(f"  an 'and' held over a chord change {fmt(ch['held_and_into_change'])}% (n={ch['ands_before_changes']}), over a bar line without one {fmt(ch['held_and_into_bar_without_change'])}% (n={ch['ands_before_bars']})")
        f = s["form"]
        out.append("  notes/bar by bar of an 8-bar section x  " + "  ".join(f"{k + 1}:{fmt(v)}" for k, v in enumerate(f["section_bars"])) + f"   turnaround (a chorus's last 2 bars) x{fmt(f['turnaround'])}")
        for b, h in s["habits"].items():
            out.append(f"  habits {b:<8} steps {fmt(h['steps'])}%  leaps {fmt(h['leaps'])}%  notes in 4+ runs {fmt(h['in_runs'])}%  3rds in runs {fmt(h['thirds_in_runs'])}%  (n={h['intervals']})")
        d = s["dynamics"]
        out.append(f"  dynamics (n={d['notes']}): low/mid/high register " + "/".join(fmt(v) for v in d["register"].values()) + " dB; phrase first/early/middle/late/last " + "/".join(fmt(v) for v in d["phrase"].values()) + " dB; chorus 1/2/3/4+ " + "/".join(fmt(v) for v in d["chorus"].values()) + " dB")
        out.append("  articulation " + "  ".join(f"{b}: legato {fmt(a['legato'])}, ghosted off-beats {fmt(a['ghosted_offbeats'])}% (n={a['offbeats']})" for b, a in s["articulation"].items()))
        out.append("")
    return "\n".join(out)


# --- a self-test on a made-up solo ------------------------------------------------------------


def selftest():
    assert band_of(120) == 0 and band_of(200) == 2 and band_of(300) == 3
    assert abs(quantile([0, 10], 0.5) - 5) < 1e-9
    db = sqlite3.connect(":memory:")
    db.executescript(
        """
        CREATE TABLE solo_info (melid, trackid, recordid, performer, signature, rhythmfeel, avgtempo);
        CREATE TABLE track_info (trackid, recordingdate);
        CREATE TABLE melody (eventid, melid, onset, duration, pitch, bar, beat, tatum, division, loud_med);
        CREATE TABLE beats (melid, onset, bar, beat, chord, form, chorus_id);
        CREATE TABLE sections (melid, type, start, end, value);
        INSERT INTO solo_info VALUES (1, 1, 1, 'Miles Davis', '4/4', 'SWING', 120);
        INSERT INTO track_info VALUES (1, '1956-10-26');
        """
    )
    # 4 bars at 120 bpm (0.5 s a beat), F7 then Bb7 at bar 3; chorus 1 is bars 1-2, chorus 2 bars 3-4.
    for b in range(16):
        bar, beat = b // 4 + 1, b % 4 + 1
        chord = "F7" if b == 0 else "Bb7" if b == 8 else ""
        db.execute("INSERT INTO beats VALUES (1, ?, ?, ?, ?, ?, ?)", (b * 0.5, bar, beat, chord, "A1" if b == 0 else "", 1 if b < 8 else 2))
    # Swung 8ths, 2:1 (the off-beat at 2/3 of the beat), 10 ms late on the beat, climbing a step
    # at a time; the last 8th of bar 2 (the "and" of 4) held over the change; loud on the beat.
    k = 0
    for b in range(16):
        bar, beat = b // 4 + 1, b % 4 + 1
        for tat, at in ((1, 0.01), (2, 0.5 / 1.5)):
            dur = 0.6 if b == 7 and tat == 2 else 0.12
            db.execute("INSERT INTO melody VALUES (?, 1, ?, ?, ?, ?, ?, ?, 2, ?)", (100 + k, b * 0.5 + at, dur, 60 + k % 12, bar, beat, tat, 70 if tat == 1 else 58))
            k += 1
    db.execute("INSERT INTO sections VALUES (1, 'PHRASE', 0, 15, '')")
    db.execute("INSERT INTO sections VALUES (1, 'PHRASE', 16, 31, '')")
    r = analyse(db)["miles"]
    t = r["timing"]["<140"]
    assert t["swing"] == 2.0 and t["lay_back_ms"] == 10.0, t
    assert r["chords"]["held_and_into_change"] == 100.0, r["chords"]
    assert r["register"]["later_choruses"] == 1, r["register"]
    assert r["articulation"]["<140"]["ghosted_offbeats"] == 100.0, r["articulation"]  # 6 dB under the phrase's median
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
