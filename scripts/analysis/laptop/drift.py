import sqlite3, re, sys, statistics as st
from collections import defaultdict
from fractions import Fraction

c = sqlite3.connect(sys.argv[1])
L = []
P = L.append


def year_of(s):
    m = re.findall(r"(1[89]\d\d|20\d\d)", s or "")
    return int(m[-1]) if m else None


def solo(melid):
    beat_on = {}
    bl = []
    for bar, beat, on in c.execute("select bar,beat,onset from beats where melid=? order by onset", (melid,)):
        beat_on[(bar, beat)] = on
        bl.append(on)
    notes = list(c.execute(
        "select onset,duration,bar,beat,tatum,division,beatdur from melody where melid=? order by eventid", (melid,)))
    phrases = list(c.execute("select start,end from sections where melid=? and type='PHRASE'", (melid,)))
    ph_of = {}
    for s, e in phrases:
        for k in range(s, e + 1):
            ph_of[k] = (s, e)
    rows = []
    for i, (on, d, bar, beat, tat, div, bd) in enumerate(notes):
        if tat != 1 or (bar, beat) not in beat_on or not bd:
            continue
        off = on - beat_on[(bar, beat)]
        if abs(off) > 0.25 * bd:  # not plausibly "on" that beat
            continue
        prev_ioi = (on - notes[i - 1][0]) / bd if i else None
        s, e = ph_of.get(i, (None, None))
        role = "first" if i == s else "last" if i == e else "inner" if s is not None else None
        rows.append(dict(ms=1000 * off, fr=off / bd, t=on, bd=bd, prev=prev_ioi, role=role))
    return rows, bl


solos = []
for melid, perf, title, rdate, tempo in c.execute(
        "select s.melid,s.performer,s.title,t.recordingdate,s.avgtempo from solo_info s join track_info t using(trackid)"):
    rows, bl = solo(melid)
    if len(rows) < 20 or len(bl) < 16:
        continue
    t0, t1 = rows[0]["t"], rows[-1]["t"]
    for r in rows:
        r["third"] = min(2, int(3 * (r["t"] - t0) / max(1e-9, t1 - t0)))
    k = max(4, len(bl) // 5)
    tempo_a = 60 / st.median([b - a for a, b in zip(bl[:k], bl[1:k])])
    tempo_b = 60 / st.median([b - a for a, b in zip(bl[-k:], bl[-k + 1:])])
    solos.append(dict(melid=melid, perf=perf, title=title, year=year_of(rdate), tempo=tempo or 0, rows=rows,
                      med=st.median(r["ms"] for r in rows), medfr=st.median(r["fr"] for r in rows),
                      tdrift=100 * (tempo_b / tempo_a - 1)))

GROUPS = [
    ("Louis Armstrong", lambda s: s["perf"] == "Louis Armstrong"),
    ("Charlie Parker", lambda s: s["perf"] == "Charlie Parker"),
    ("Miles Davis 1951-59", lambda s: s["perf"] == "Miles Davis" and s["year"] and 1951 <= s["year"] <= 1959),
    ("Miles Davis 1964-69", lambda s: s["perf"] == "Miles Davis" and s["year"] and 1964 <= s["year"] <= 1969),
    ("Sonny Rollins", lambda s: s["perf"] == "Sonny Rollins"),
    ("John Coltrane <1960", lambda s: s["perf"] == "John Coltrane" and s["year"] and s["year"] < 1960),
    ("John Coltrane 1960+", lambda s: s["perf"] == "John Coltrane" and s["year"] and s["year"] >= 1960),
    ("Wayne Shorter", lambda s: s["perf"] == "Wayne Shorter"),
    ("whole database", lambda s: True),
]


def m(xs, f="%+.0f"):
    return f % st.mean(xs) if xs else "  n/a"


P("TIMING DRIFT -- Weimar Jazz Database (wjazzd.db). Soloist's notes that sit ON a beat")
P("(first tatum of the beat), measured against the database's annotated beat times")
P("(beats table). + = behind the beat (late), - = ahead. Notes more than 1/4 beat off")
P("are dropped. ms = milliseconds; %b = percent of a beat. Solos with < 20 on-beat notes")
P("skipped (%d solos kept). Caveat: the beat track is hand-annotated from the recording" % len(solos))
P("(mostly following the rhythm section), so this is 'soloist vs band pulse as heard',")
P("with a few ms of annotation noise.")
P("")
P("1. WHERE THEY SIT (per-solo medians, then summarised)")
P("%-21s %5s %7s %9s %7s %16s %8s" % ("player", "solos", "ms", "%beat", "SD ms", "range of solos", "all ahead/behind"))
for name, f in GROUPS:
    ss = [s for s in solos if f(s)]
    meds = [s["med"] for s in ss]
    allr = [r["ms"] for s in ss for r in s["rows"]]
    behind = sum(x > 0 for x in meds)
    P("%-21s %5d %+7.0f %+8.1f%% %7.0f %7.0f..%+.0f ms %5d/%d behind" % (
        name, len(ss), st.median(meds), 100 * st.median(s["medfr"] for s in ss), st.pstdev(allr),
        min(meds), max(meds), behind, len(ss)))
P("")

P("2. DRIFT INSIDE A PHRASE (mean ms: on-beat phrase-first note / inner on-beat notes / phrase-last note)")
P("   and by what came before (previous note 16th-or-faster / 8th-ish / after a quarter or longer)")
P("%-21s %7s %7s %7s   %7s %7s %7s" % ("player", "first", "inner", "last", "fast", "8th", "long"))
for name, f in GROUPS:
    rs = [r for s in solos if f(s) for r in s["rows"]]
    by = lambda k: [r["ms"] for r in rs if r["role"] == k]
    pv = lambda lo, hi: [r["ms"] for r in rs if r["prev"] is not None and lo <= r["prev"] < hi]
    P("%-21s %7s %7s %7s   %7s %7s %7s" % (name, m(by("first")), m(by("inner")), m(by("last")),
                                          m(pv(0, 0.3)), m(pv(0.3, 0.7)), m(pv(0.95, 99))))
P("")

P("3. DRIFT ACROSS THE SOLO (mean ms in the first / middle / last third of each solo) and")
P("   the band's tempo change from the first to the last fifth of the solo (median %)")
P("%-21s %7s %7s %7s %10s %22s" % ("player", "1st", "2nd", "3rd", "tempo", "solos later in 3rd"))
for name, f in GROUPS:
    ss = [s for s in solos if f(s)]
    th = [[r["ms"] for s in ss for r in s["rows"] if r["third"] == k] for k in range(3)]
    later = sum(st.mean([r["ms"] for r in s["rows"] if r["third"] == 2] or [0]) >
                st.mean([r["ms"] for r in s["rows"] if r["third"] == 0] or [0]) for s in ss)
    P("%-21s %7s %7s %7s %+9.1f%% %15d/%d" % (name, m(th[0]), m(th[1]), m(th[2]),
                                              st.median(s["tdrift"] for s in ss), later, len(ss)))
P("")

# tempo dependence
xs = [s["tempo"] for s in solos]
P("4. TEMPO (whole database, one point per solo)")
for lo, hi in ((0, 120), (120, 180), (180, 240), (240, 400)):
    ss = [s for s in solos if lo <= s["tempo"] < hi]
    if ss:
        P("   %3d-%3d bpm: %3d solos, median %+4.0f ms = %+5.1f%% of a beat" % (
            lo, hi, len(ss), st.median(s["med"] for s in ss), 100 * st.median(s["medfr"] for s in ss)))
P("")

# performer effect
by_p = defaultdict(list)
for s in solos:
    by_p[s["perf"]].append(s["med"])
big = {p: v for p, v in by_p.items() if len(v) >= 4}
allv = [x for v in big.values() for x in v]
gm = st.mean(allv)
ssb = sum(len(v) * (st.mean(v) - gm) ** 2 for v in big.values())
sst = sum((x - gm) ** 2 for x in allv)
P("5. IS IT A PERSONAL TENDENCY? Performers with >= 4 solos (%d players, %d solos):" % (len(big), len(allv)))
P("   share of the solo-to-solo variation in median offset explained by who is playing")
P("   (eta squared): %.0f%%  (0%% = no personal habit, 100%% = fully personal)" % (100 * ssb / sst))
P("   most behind / most ahead (median of their solo medians, ms; solos; how many behind):")
rank = sorted(big.items(), key=lambda kv: -st.median(kv[1]))
for p, v in rank[:8]:
    P("     %-24s %+5.0f  (%d solos, %d behind)" % (p, st.median(v), len(v), sum(x > 0 for x in v)))
P("     ...")
for p, v in rank[-8:]:
    P("     %-24s %+5.0f  (%d solos, %d behind)" % (p, st.median(v), len(v), sum(x > 0 for x in v)))
print("\n".join(L))
