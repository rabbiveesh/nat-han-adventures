#!/usr/bin/env python3
"""How our soloists plan a solo: the arc, ideas over phrases, the skeleton, tempo, entries.

Checks the soloist's planning choices (`soloist.rs`) against the Weimar Jazz Database v2.1, for
each of the band's measured soloists and the whole database:

  1. the arc: notes per bar, register, phrase length, rest and the share of 16th-or-faster
     gaps by fifths of each chorus and of each solo; where the top note falls; each solo's
     shape (builds, front-loaded, arch, flat); the first chorus against the later ones;
  2. ideas over phrases: a phrase restating the one before (its first 4 intervals within a
     semitone each), how often a restatement is restated again (chains), how much of it, and
     its transposition against the root motion between the two phrases' first chords; and
     ideas carried: the head's rhythm or contour repeated, or a 4-interval cell shared anywhere,
     against chance (with the excess as a carry probability), and where a shaped shared cell
     sits, whether its rhythm is kept and how it is moved;
  3. the skeleton: the degree sounding on chord changes on beats 1 and 3 inside phrases, the
     voice leading between those notes, and the phrases' contours;
  4. tempo: the share of 16th-or-faster gaps and notes per bar by tempo band;
  5. section entries: phrase starts within a beat after an 8-bar section line, against chance.

Phrases are the database's hand-marked ones; a "gap" is the onset-to-onset time inside a
phrase, in beats (16th-or-faster: under 0.29 beat; a triplet 8th is 0.33). Statistics only:
nothing here copies a phrase. Standard library only:

  python3 -I scripts/analysis/wjazzd_strategies.py wjazzd.db > strategies.txt
  python3 -I scripts/analysis/wjazzd_strategies.py wjazzd.db --json strategies.json
  python3 -I scripts/analysis/wjazzd_strategies.py --selftest           # no database
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

FAST = 0.29  # beats
BANDS = [(0, 140), (140, 200), (200, 260), (260, 1000)]
SHAPES = ["builds", "arch", "front-loaded", "flat"]
CONTOURS = ["arch", "up", "down", "flat", "other"]
SKELETON = ["3", "7", "5", "R", "9", "other"]


def mean(v):
    return statistics.mean(v) if v else None


def r(x, n=2):
    return None if x is None else round(x, n)


# --- one solo -----------------------------------------------------------------------------


class Solo:
    """A solo's notes, beat grid, phrases and choruses, with each note's time in beats."""

    def __init__(self, melid, notes, beats, phrases, choruses, tempo):
        self.melid, self.notes, self.tempo = melid, notes, tempo
        self.grid = E.Beats(beats)
        self.phrases = phrases
        self.choruses = choruses
        self.b = [self.at(n[1]) for n in notes]  # each note's onset, in beats from the first beat

    def at(self, t):
        g = self.grid
        i = g.index(t)
        if i < 0:
            return (t - g.on[0]) / max(1e-6, g.beat_len(0))
        bl = g.beat_len(i)
        return i + ((t - g.on[i]) / bl if bl > 0 else 0.0)

    def end(self, k):
        n = self.notes[k]
        return self.at(n[1] + (n[2] or 0))

    def gaps(self, a, z):
        return [self.b[k + 1] - self.b[k] for k in range(a, z)]


def load(db, melid, tempo):
    notes, beats, secs = E.load(db, melid)
    if not notes or len(beats) < 8 or any(n[3] is None for n in notes):
        return None
    phrases = spans(notes, secs)
    rows = db.execute("SELECT start, end, value FROM sections WHERE melid=? AND type='CHORUS' ORDER BY start", (melid,)).fetchall()
    choruses = [(s, e) for s, e in spans(notes, [(s, e) for s, e, v in rows if str(v).lstrip("-").isdigit() and int(v) >= 1])]
    return Solo(melid, notes, beats, phrases, choruses, tempo or 0)


def spans(notes, secs):
    """(first, last) note indexes (sections count notes from 0 within the solo, or eventids)."""
    if not secs:
        return []
    if max(e for _, e in secs) < len(notes):
        return [(s, e) for s, e in secs if 0 <= s <= e < len(notes)]
    ids = {n[0]: i for i, n in enumerate(notes)}
    return [(ids[s], ids[e]) for s, e in secs if s in ids and e in ids]


# --- 1. the arc --------------------------------------------------------------------------


def fifths(s, a, z):
    """Notes a..=z by fifths of their span in beats: per fifth, (notes per bar, mean pitch,
    mean phrase length in beats, mean rest before a phrase in beats, the fast share), each
    phrase counted in the fifth it starts in."""
    t0, t1 = s.b[a], max(s.end(z), s.b[z] + 0.25)
    span = t1 - t0
    if span < 8:
        return None
    out = []
    starts = {p[0]: p for p in s.phrases}
    prev_end = {}
    for j, (pa, pz) in enumerate(s.phrases):
        if j:
            prev_end[pa] = s.end(s.phrases[j - 1][1])
    for k in range(5):
        lo, hi = t0 + span * k / 5, t0 + span * (k + 1) / 5
        idx = [i for i in range(a, z + 1) if lo <= s.b[i] < hi]
        ph = [starts[i] for i in idx if i in starts]
        fast = tot = 0
        in_phrase = set()
        for pa, pz in s.phrases:
            for i in range(pa, pz):
                in_phrase.add(i)
        for i in idx:
            if i in in_phrase and i + 1 < len(s.b):
                tot += 1
                fast += s.b[i + 1] - s.b[i] < FAST
        out.append(dict(
            density=len(idx) / ((hi - lo) / 4),
            pitch=mean([s.notes[i][3] for i in idx]),
            phrase=mean([s.end(pz) - s.b[pa] for pa, pz in ph]),
            rest=mean([s.b[pa] - prev_end[pa] for pa, pz in ph if pa in prev_end]),
            fast=fast / tot if tot else None,
        ))
    return out


def relative(rows):
    """A solo's fifths against its own mean: ratios for density, phrase, rest and fast; a
    difference in semitones for pitch."""
    out = []
    keys = ("density", "phrase", "rest", "fast")
    m = {k: mean([x[k] for x in rows if x[k] is not None]) for k in keys + ("pitch",)}
    for x in rows:
        y = {}
        for k in keys:
            y[k] = x[k] / m[k] if x[k] is not None and m[k] else None
        y["pitch"] = x["pitch"] - m["pitch"] if x["pitch"] is not None and m["pitch"] is not None else None
        out.append(y)
    return out


def shape(rel):
    """builds / front-loaded / arch / flat, from density and register by fifths together."""
    inten = []
    for x in rel:
        if x["density"] is None or x["pitch"] is None:
            return None
        inten.append((x["density"] - 1) + x["pitch"] / 6)
    if max(inten) - min(inten) < 0.25:
        return "flat"
    p = max(range(5), key=lambda k: inten[k])
    if p == 4 or (p == 3 and inten[4] > inten[0] + 0.1):
        return "builds"
    if p == 0 or (p == 1 and inten[0] > inten[4] + 0.1):
        return "front-loaded"
    return "arch"


# --- 2. ideas over phrases -----------------------------------------------------------------


def intervals(s, a, z):
    return [s.notes[k + 1][3] - s.notes[k][3] for k in range(a, z)]


def matched(x, y):
    """How many leading intervals of x and y agree within a semitone."""
    n = 0
    for p, q in zip(x, y):
        if abs(p - q) > 1:
            break
        n += 1
    return n


def chord_root(s, k):
    i = s.grid.index(s.notes[k][1])
    c = s.grid.chord[i] if i >= 0 else None
    return c[0] if c else None


def shaped(iv):
    """A cell with a leap (3 or more) or a turn in it: an idea, not a plain scale run."""
    return any(abs(i) >= 3 for i in iv) or any(a * b < 0 for a, b in zip(iv, iv[1:]))


def shared_cell(x, y, need_shape=False):
    """The first 4-interval cell of x found in y (each interval within a semitone): (i, j) or
    None."""
    for i in range(len(x) - 3):
        c = x[i : i + 4]
        if need_shape and not shaped(c):
            continue
        for j in range(len(y) - 3):
            if all(abs(a - b) <= 1 for a, b in zip(c, y[j : j + 4])):
                return i, j
    return None


def sign(v):
    return [(x > 0) - (x < 0) for x in v]


def repeats(s, p, q):
    """How phrase q repeats phrase p: its head's intervals, rhythm or contour, or a cell anywhere."""
    (pa, pz), (qa, qz) = p, q
    x, y = intervals(s, pa, pz), intervals(s, qa, qz)
    rx, ry = s.gaps(pa, pz), s.gaps(qa, qz)
    rhythm = len(rx) >= 4 and len(ry) >= 4 and all(abs(a - b) <= 0.09 for a, b in zip(rx[:4], ry[:4])) and abs(s.b[pa] % 1 - s.b[qa] % 1) < 0.1
    return {"head": matched(x, y) >= 4, "rhythm": rhythm, "contour": len(x) >= 5 and len(y) >= 5 and sign(x[:5]) == sign(y[:5]), "cell": shared_cell(x, y) is not None}


REPEATS = ["head", "rhythm", "contour", "cell"]


# --- 3. the skeleton ---------------------------------------------------------------------


def skel_fold(d):
    if d in ("3", "7", "5", "R"):
        return d
    if d in ("9", "b9", "#9"):
        return "9"
    return "other"


def contour(pitches):
    first, last, top = pitches[0], pitches[-1], max(pitches)
    if top - min(pitches) <= 2:
        return "flat"
    if top > first + 2 and top > last + 2:
        return "arch"
    if last > first + 2:
        return "up"
    if last < first - 2:
        return "down"
    return "other"


# --- the measurements -------------------------------------------------------------------


class Acc:
    def __init__(self):
        self.c = collections.Counter()
        self.lists = collections.defaultdict(list)
        self.grids = collections.defaultdict(lambda: [[] for _ in range(5)])


def tally(a, s, rng_seed=0):
    # 1. The arc, per chorus (solos with choruses marked) and over the whole solo.
    for (ca, cz) in s.choruses:
        f = fifths(s, ca, cz)
        if f:
            for k, x in enumerate(relative(f)):
                for key, v in x.items():
                    if v is not None:
                        a.grids["chorus_" + key][k].append(v)
    f = fifths(s, 0, len(s.notes) - 1)
    if f:
        rel = relative(f)
        for k, x in enumerate(rel):
            for key, v in x.items():
                if v is not None:
                    a.grids["solo_" + key][k].append(v)
        sh = shape(rel)
        if sh:
            a.c["shape_" + sh] += 1
            a.c["shape_total"] += 1
    top = max(range(len(s.notes)), key=lambda k: s.notes[k][3])
    t0, t1 = s.b[0], s.end(len(s.notes) - 1)
    if t1 > t0:
        a.lists["top_in_solo"].append((s.b[top] - t0) / (t1 - t0))
    for ca, cz in s.choruses:
        if ca <= top <= cz:
            u0, u1 = s.b[ca], s.end(cz)
            if u1 > u0:
                a.lists["top_in_chorus"].append((s.b[top] - u0) / (u1 - u0))
    # The first chorus against the later ones (solos of two choruses or more).
    if len(s.choruses) >= 2:
        per = []
        for ca, cz in s.choruses:
            ph = [(pa, pz) for pa, pz in s.phrases if ca <= pa <= cz]
            gaps = [g for pa, pz in ph for g in s.gaps(pa, pz)]
            bars = max(1e-6, (s.end(cz) - s.b[ca]) / 4)
            per.append(dict(density=(cz - ca + 1) / bars, pitch=mean([s.notes[k][3] for k in range(ca, cz + 1)]),
                            phrase=mean([s.end(pz) - s.b[pa] for pa, pz in ph]), fast=sum(g < FAST for g in gaps) / max(1, len(gaps))))
        later = per[1:]
        for key in ("density", "phrase", "fast"):
            lm = mean([x[key] for x in later if x[key] is not None])
            if per[0][key] is not None and lm:
                a.lists["first_vs_later_" + key].append(per[0][key] / lm)
        lp = mean([x["pitch"] for x in later])
        if per[0]["pitch"] is not None and lp is not None:
            a.lists["first_vs_later_pitch"].append(per[0]["pitch"] - lp)
    # 2. Ideas over phrases.
    ph = [(pa, pz) for pa, pz in s.phrases if pz - pa >= 4]
    chain = 0
    prev_restated = None
    for j in range(1, len(ph)):
        (pa, pz), (qa, qz) = ph[j - 1], ph[j]
        x, y = intervals(s, pa, pz), intervals(s, qa, qz)
        m = matched(x, y)
        restated = m >= 4
        a.c["pairs"] += 1
        a.c["restated"] += restated
        # Chance: against a phrase further back in the same solo.
        if j >= 4:
            b = ph[(j * 7 + rng_seed) % (j - 2)]
            a.c["chance_pairs"] += 1
            a.c["chance_restated"] += matched(intervals(s, b[0], b[1]), y) >= 4
        if prev_restated is not None:
            a.c["after_restated" if prev_restated else "after_new"] += 1
            if restated:
                a.c["again_after_restated" if prev_restated else "again_after_new"] += 1
        if restated:
            chain += 1
            shorter = min(len(x), len(y))
            a.lists["restated_notes"].append(m + 1)
            a.c["restated_all" if m >= 0.75 * shorter else "restated_head"] += 1
            moved = (s.notes[qa][3] - s.notes[pa][3]) % 12
            r0, r1 = chord_root(s, pa), chord_root(s, qa)
            if r0 is not None and r1 is not None:
                motion = (r1 - r0) % 12
                if motion == 0:
                    a.c["same_chord"] += 1
                    a.c["same_chord_at_0" if moved == 0 else "same_chord_moved"] += 1
                else:
                    a.c["new_chord"] += 1
                    a.c["new_chord_with_root" if moved == motion else "new_chord_at_0" if moved == 0 else "new_chord_other"] += 1
        else:
            if chain:
                a.c["chain_%d" % min(chain, 4)] += 1
            chain = 0
        prev_restated = restated
    if chain:
        a.c["chain_%d" % min(chain, 4)] += 1
    # 2b. Ideas carried across phrases (phrases of 6 notes or more): the next phrase repeating
    # the last one's head (intervals, rhythm, contour) or sharing a 4-interval cell anywhere,
    # against a phrase from further back (chance) and the one two back; how often a repeat is
    # followed by another (chains); and a shaped shared cell's place, rhythm and move.
    long_ph = [(pa, pz) for pa, pz in s.phrases if pz - pa >= 5]
    run = dict.fromkeys(REPEATS, 0)
    last = None
    for j in range(1, len(long_ph)):
        t = repeats(s, long_ph[j - 1], long_ph[j])
        a.c["rep_pairs"] += 1
        for k in REPEATS:
            a.c["rep_" + k] += t[k]
            if last is not None and last[k]:
                a.c["rep_after_" + k] += 1
                a.c["rep_again_" + k] += t[k]
            if t[k]:
                run[k] += 1
            elif run[k]:
                a.c["rep_chain_%s_%d" % (k, min(run[k], 4))] += 1
                run[k] = 0
        last = t
        if j >= 2:
            t2 = repeats(s, long_ph[j - 2], long_ph[j])
            a.c["rep_lag2_pairs"] += 1
            for k in REPEATS:
                a.c["rep_lag2_" + k] += t2[k]
        if j >= 5:
            tc = repeats(s, long_ph[(j * 7 + rng_seed) % (j - 3)], long_ph[j])
            a.c["rep_chance_pairs"] += 1
            for k in REPEATS:
                a.c["rep_chance_" + k] += tc[k]
        (pa, pz), (qa, qz) = long_ph[j - 1], long_ph[j]
        found = shared_cell(intervals(s, pa, pz), intervals(s, qa, qz), need_shape=True)
        if found:
            i, jj = found
            a.c["carried"] += 1
            a.lists["carried_at"].append(jj / max(1, (qz - qa) - 3))
            a.c["carried_rhythm"] += all(abs(g - h) <= 0.09 for g, h in zip(s.gaps(pa + i, pa + i + 4), s.gaps(qa + jj, qa + jj + 4)))
            moved = (s.notes[qa + jj][3] - s.notes[pa + i][3]) % 12
            r0, r1 = chord_root(s, pa + i), chord_root(s, qa + jj)
            if r0 is None or r1 is None:
                a.c["carried_no_chord"] += 1
            elif (r1 - r0) % 12 == 0:
                a.c["carried_same_at_0" if moved == 0 else "carried_same_moved"] += 1
            else:
                a.c["carried_new_root" if moved == (r1 - r0) % 12 else "carried_new_at_0" if moved == 0 else "carried_new_other"] += 1
    for k in REPEATS:
        if run[k]:
            a.c["rep_chain_%s_%d" % (k, min(run[k], 4))] += 1
    # 3. The skeleton: chord changes on 1 and 3 inside phrases.
    g = s.grid
    for pa, pz in s.phrases:
        t0, t1 = s.notes[pa][1], s.notes[pz][1] + (s.notes[pz][2] or 0)
        held = []
        for i, on in enumerate(g.on):
            if not (t0 <= on <= t1) or not g.change[i] or g.pos[i][1] not in (1, 3) or g.chord[i] is None:
                continue
            tol = 0.125 * g.beat_len(i)
            k = max((k for k in range(pa, pz + 1) if s.notes[k][1] <= on + tol), default=None)
            if k is None:
                continue
            n = s.notes[k]
            if n[1] + (n[2] or 0) < on - tol and n[1] < on - tol:
                continue  # a rest there
            d, kind = E.degree(n[3], g.chord[i])
            a.c["skel_" + skel_fold(d)] += 1
            a.c["skel_total"] += 1
            held.append(n[3])
        for p, q in zip(held, held[1:]):
            a.lists["skel_move"].append(abs(q - p))
        if pz - pa >= 3:
            a.c["contour_" + contour([s.notes[k][3] for k in range(pa, pz + 1)])] += 1
            a.c["contour_total"] += 1
    # 4. Tempo.
    band = next((k for k, (lo, hi) in enumerate(BANDS) if lo <= s.tempo < hi), None)
    if band is not None:
        for pa, pz in s.phrases:
            for gap in s.gaps(pa, pz):
                a.c["tempo%d_gaps" % band] += 1
                a.c["tempo%d_fast" % band] += gap < FAST
        a.c["tempo%d_notes" % band] += len(s.notes)
        a.c["tempo%d_beats" % band] += len(g.on)
    # 5. Section entries: within a beat after an 8-bar line (from the form's section starts).
    lines = set()
    for i, (bar, beat) in enumerate(g.pos):
        if beat != 1:
            continue
        start = max([b for b in g.section_bar if b <= bar], default=1)
        if (bar - start) % 8 == 0:
            lines.add(i)
    for pa, pz in s.phrases:
        x = s.b[pa]
        i = int(x)
        a.c["entries"] += 1
        a.c["entries_after_line"] += i in lines and x - i < 1.0
        a.c["entries_after_bar"] += 0 <= i < len(g.pos) and g.pos[i][1] == 1
    a.c["beats"] += len(g.on)
    a.c["line_beats"] += len(lines)
    a.c["solos"] += 1


def pct(c, k, t):
    return round(100 * c[k] / c[t], 1) if c[t] else None


def summary(a):
    c = a.c
    out = {"solos": c["solos"]}
    for scope in ("chorus", "solo"):
        out["arc_" + scope] = {key: [r(mean(a.grids[scope + "_" + key][k])) for k in range(5)] for key in ("density", "pitch", "phrase", "rest", "fast")}
    out["shapes"] = {s: pct(c, "shape_" + s, "shape_total") for s in SHAPES}
    out["shapes_n"] = c["shape_total"]
    for k in ("top_in_solo", "top_in_chorus"):
        v = a.lists[k]
        out[k] = dict(median=r(statistics.median(v)) if v else None, last_quarter=round(100 * sum(x >= 0.75 for x in v) / len(v), 1) if v else None,
                      quarters=[round(100 * sum(q / 4 <= x < (q + 1) / 4 or (q == 3 and x >= 1) for x in v) / len(v), 1) for q in range(4)] if v else None)
    out["first_vs_later"] = {k: r(statistics.median(a.lists["first_vs_later_" + k])) if a.lists["first_vs_later_" + k] else None for k in ("density", "pitch", "phrase", "fast")}
    out["first_vs_later_n"] = len(a.lists["first_vs_later_density"])
    rn = a.lists["restated_notes"]
    out["ideas"] = dict(
        pairs=c["pairs"], restated=pct(c, "restated", "pairs"), chance=pct(c, "chance_restated", "chance_pairs"),
        again_after_restated=pct(c, "again_after_restated", "after_restated"), again_after_new=pct(c, "again_after_new", "after_new"),
        chains={str(k) + ("+" if k == 4 else ""): c["chain_%d" % k] for k in range(1, 5)},
        head_only=pct(c, "restated_head", "restated"), all=pct(c, "restated_all", "restated"),
        notes_median=statistics.median(rn) if rn else None,
        new_chord=c["new_chord"], moved_with_root=pct(c, "new_chord_with_root", "new_chord"), at_0_new_chord=pct(c, "new_chord_at_0", "new_chord"), other_new_chord=pct(c, "new_chord_other", "new_chord"),
        same_chord=c["same_chord"], at_0_same_chord=pct(c, "same_chord_at_0", "same_chord"),
    )
    rep = {}
    for k in REPEATS:
        adj, chance = pct(c, "rep_" + k, "rep_pairs"), pct(c, "rep_chance_" + k, "rep_chance_pairs")
        rep[k] = dict(next=adj, two_back=pct(c, "rep_lag2_" + k, "rep_lag2_pairs"), chance=chance, again=pct(c, "rep_again_" + k, "rep_after_" + k),
                      carry=round(max(0.0, (adj - chance) / (100 - chance)), 3) if adj is not None and chance is not None and chance < 100 else None,
                      chains={str(n) + ("+" if n == 4 else ""): c["rep_chain_%s_%d" % (k, n)] for n in range(1, 5)})
    at = a.lists["carried_at"]
    out["carried"] = dict(pairs=c["rep_pairs"], repeats=rep, shaped_cells=c["carried"],
                          at_fifths=[round(100 * sum(lo / 5 <= x < (lo + 1) / 5 or (lo == 4 and x >= 1) for x in at) / len(at), 1) for lo in range(5)] if at else None,
                          rhythm_kept=pct(c, "carried_rhythm", "carried"),
                          moves={k: pct(c, "carried_" + k, "carried") for k in ("new_root", "new_at_0", "new_other", "same_moved", "same_at_0", "no_chord")})
    mv = a.lists["skel_move"]
    out["skeleton"] = dict(n=c["skel_total"], degrees={d: pct(c, "skel_" + d, "skel_total") for d in SKELETON},
                           move_median=statistics.median(mv) if mv else None, move_2_or_less=round(100 * sum(x <= 2 for x in mv) / len(mv), 1) if mv else None,
                           contours={k: pct(c, "contour_" + k, "contour_total") for k in CONTOURS}, contours_n=c["contour_total"])
    out["tempo"] = {"%d-%d" % b if b[1] < 1000 else ">%d" % b[0]: dict(fast=pct(c, "tempo%d_fast" % k, "tempo%d_gaps" % k), notes_per_bar=round(4 * c["tempo%d_notes" % k] / c["tempo%d_beats" % k], 2) if c["tempo%d_beats" % k] else None, gaps=c["tempo%d_gaps" % k]) for k, b in enumerate(BANDS)}
    out["entries"] = dict(after_8bar_line=pct(c, "entries_after_line", "entries"), chance=round(100 * c["line_beats"] / c["beats"], 1) if c["beats"] else None,
                          after_any_bar_line=pct(c, "entries_after_bar", "entries"), phrases=c["entries"])
    return out


def players():
    return [p for p in E.PLAYERS if p[0] not in ("funk", "latin")]


def analyse(db):
    rows = E.solos(db)
    tempo = dict(db.execute("SELECT melid, avgtempo FROM solo_info"))
    cache = {}
    res = {}
    for slug, perf, y0, y1, feel in players():
        a = Acc()
        for melid in E.pick(rows, perf, y0, y1, feel):
            if melid not in cache:
                cache[melid] = load(db, melid, tempo.get(melid))
            if cache[melid]:
                tally(a, cache[melid], melid)
        res[slug] = summary(a)
    return res


# --- the report -----------------------------------------------------------------------------


def fmt(v):
    return "-" if v is None else ("%g" % v)


def report(res):
    L = [
        "Soloist strategies in the Weimar Jazz Database v2.1, by player (hand-marked phrases; a gap is",
        "onset to onset inside a phrase, 16th-or-faster under 0.29 beat). Arc rows: each solo's (or",
        "chorus's) fifths against its own mean (x for ratios, semitones for pitch), averaged over solos.",
        "",
    ]
    for slug, s in res.items():
        L.append(f"{slug}: {s['solos']} solos")
        for scope in ("chorus", "solo"):
            a = s["arc_" + scope]
            for key, label in (("density", "notes/bar x"), ("pitch", "pitch +st"), ("phrase", "phrase len x"), ("rest", "rest x"), ("fast", "16ths x")):
                L.append(f"  arc by {scope:<6} {label:<13}" + "  ".join(f"{fmt(v):>5}" for v in a[key]))
        L.append(f"  solo shapes (n={s['shapes_n']})      " + "  ".join(f"{k} {fmt(v)}" for k, v in s["shapes"].items()))
        for k in ("top_in_chorus", "top_in_solo"):
            t = s[k]
            L.append(f"  top note, {k[7:]:<7}         median {fmt(t['median'])}, by quarter {t['quarters']}")
        f = s["first_vs_later"]
        L.append(f"  first chorus / later (n={s['first_vs_later_n']})  notes/bar x{fmt(f['density'])}  phrase x{fmt(f['phrase'])}  16ths x{fmt(f['fast'])}  pitch {fmt(f['pitch'])} st")
        i = s["ideas"]
        L.append(f"  restates the last phrase   {fmt(i['restated'])}% of {i['pairs']} (chance {fmt(i['chance'])}%); again after a restatement {fmt(i['again_after_restated'])}%, after a new one {fmt(i['again_after_new'])}%")
        L.append(f"  restatement chains          " + "  ".join(f"{k}: {v}" for k, v in i["chains"].items()) + f";  head only {fmt(i['head_only'])}%, nearly all {fmt(i['all'])}%; notes matched median {fmt(i['notes_median'])}")
        L.append(f"  moved, on a new chord (n={i['new_chord']}) with the root {fmt(i['moved_with_root'])}%, not moved {fmt(i['at_0_new_chord'])}%, other {fmt(i['other_new_chord'])}%; same chord (n={i['same_chord']}) not moved {fmt(i['at_0_same_chord'])}%")
        cr = s["carried"]
        L.append(f"  ideas carried (n={cr['pairs']} pairs of 6+ note phrases): the next phrase repeats the last one's")
        for name, label in (("head", "head intervals"), ("rhythm", "head rhythm"), ("contour", "head contour"), ("cell", "a cell anywhere")):
            x = cr["repeats"][name]
            L.append(f"    {label:<16} {fmt(x['next'])}% (two back {fmt(x['two_back'])}%, chance {fmt(x['chance'])}%; carry {fmt(x['carry'])}); again after one {fmt(x['again'])}%; chains " + " ".join(f"{k}:{v}" for k, v in x["chains"].items()))
        mv = cr["moves"]
        L.append(f"    shaped cells shared (n={cr['shaped_cells']}): where, by fifths of the phrase {cr['at_fifths']}; rhythm kept {fmt(cr['rhythm_kept'])}%")
        L.append(f"    moved: new chord with the root {fmt(mv['new_root'])}%, not moved {fmt(mv['new_at_0'])}%, other {fmt(mv['new_other'])}%; same chord moved {fmt(mv['same_moved'])}%, not moved {fmt(mv['same_at_0'])}%; no chord {fmt(mv['no_chord'])}%")
        k = s["skeleton"]
        L.append(f"  on changes at 1 and 3 (n={k['n']})  " + "  ".join(f"{d} {fmt(v)}" for d, v in k["degrees"].items()) + f";  move median {fmt(k['move_median'])} st, 2 or less {fmt(k['move_2_or_less'])}%")
        L.append(f"  contours (n={k['contours_n']})          " + "  ".join(f"{d} {fmt(v)}" for d, v in k["contours"].items()))
        L.append("  by tempo                   " + "  ".join(f"{b}: 16ths {fmt(t['fast'])}% {fmt(t['notes_per_bar'])}/bar" for b, t in s["tempo"].items()))
        e = s["entries"]
        L.append(f"  starts within a beat after an 8-bar line {fmt(e['after_8bar_line'])}% (chance {fmt(e['chance'])}%); in a bar's first beat {fmt(e['after_any_bar_line'])}% (chance 25)")
        L.append("")
    return "\n".join(L)


# --- a self-test on a made-up solo ------------------------------------------------------------


def selftest():
    assert matched([2, 2, -1, 3, 5], [2, 1, -1, 4, 9]) == 4 and matched([2, 2], [5, 2]) == 0
    assert contour([60, 64, 67, 62]) == "arch" and contour([60, 62, 65]) == "up" and contour([67, 64, 60]) == "down" and contour([60, 61, 60]) == "flat"
    rel = [dict(density=d, pitch=p, phrase=1, rest=1, fast=1) for d, p in ((0.8, -2), (0.9, -1), (1.0, 0), (1.2, 2), (1.1, 1))]
    assert shape(rel) == "builds", shape(rel)
    assert shape([dict(x, density=1.0, pitch=0) for x in rel]) == "flat"
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
    # 16 bars at 120 bpm: F7 for 8 bars, then Bb7 (a change on beat 1 of bar 9).
    for b in range(64):
        bar, beat = b // 4 + 1, b % 4 + 1
        chord = "F7" if b == 0 else "Bb7" if b == 32 else ""
        db.execute("INSERT INTO beats VALUES (1, ?, ?, ?, ?, ?)", (b * 0.5, bar, beat, chord, "A1" if b == 0 else ""))
    notes = []
    # Phrase 1, bar 1: up from A, the 3rd of F7; phrase 2, bar 5: the same shape; phrase 3 on
    # bar 9's downbeat, the change: the same shape a 4th up, from D (the 3rd of Bb7).
    for start, base in ((0.0, 69), (8.0, 69), (16.0, 74)):
        for k, step in enumerate((0, 3, 5, 8, 7)):
            notes.append((start + k * 0.5, 0.25 if k < 4 else 1.0, base + step))
    for i, (on, du, p) in enumerate(notes):
        bar = int(on // 2) + 1
        beat = int(on % 2 // 0.5) + 1
        db.execute("INSERT INTO melody VALUES (?, 1, ?, ?, ?, ?, ?, 1, 2)", (100 + i, on, du, p, bar, beat))
    for s, e in ((0, 4), (5, 9), (10, 14)):
        db.execute("INSERT INTO sections VALUES (1, 'PHRASE', ?, ?, '')", (s, e))
    db.execute("INSERT INTO sections VALUES (1, 'CHORUS', 0, 14, '1')")
    r = analyse(db)["miles"]
    assert r["solos"] == 1, r
    i = r["ideas"]
    assert i["pairs"] == 2 and i["restated"] == 100.0, i
    assert i["chains"]["2"] == 1, i["chains"]
    assert i["new_chord"] == 1 and i["moved_with_root"] == 100.0, i  # up a 4th, F7 to Bb7
    assert i["same_chord"] == 1 and i["at_0_same_chord"] == 100.0, i
    assert r["skeleton"]["n"] == 2 and r["skeleton"]["degrees"]["3"] == 100.0, r["skeleton"]  # bar 1 and bar 9
    assert r["skeleton"]["contours"]["up"] == 100.0, r["skeleton"]
    assert r["entries"]["after_8bar_line"] == 66.7, r["entries"]  # on bar 1's ONE and bar 9's
    assert shared_cell([1, 2, 3, 4, 5, -3, 4, 2], [7, 7, 5, -2, 4, 2, 9]) == (4, 2) and shaped([5, -3, 4, 2]) and not shaped([2, 2, 1, 2])
    cr = r["carried"]
    assert cr["pairs"] == 0, cr  # (the test phrases are 5 notes: too short to count)
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
