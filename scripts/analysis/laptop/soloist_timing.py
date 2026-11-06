import sqlite3, re, sys, statistics as st
from collections import defaultdict
from fractions import Fraction

c = sqlite3.connect(sys.argv[1])
L = []
P = L.append


def year_of(s):
    m = re.findall(r"(1[89]\d\d|20\d\d)", s or "")
    return int(m[-1]) if m else None


def linfit(xs, ys):
    mx, my = st.mean(xs), st.mean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    return (sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx) if sxx else 0.0


def load(melid):
    beats = list(c.execute("select bar,beat,onset,chorus_id from beats where melid=? order by onset", (melid,)))
    idx = {(b[0], b[1]): i for i, b in enumerate(beats)}
    notes = list(c.execute(
        "select onset,duration,bar,beat,tatum,division,beatdur,num from melody where melid=? order by eventid", (melid,)))
    phrases = list(c.execute("select start,end from sections where melid=? and type='PHRASE'", (melid,)))
    pos = {}
    for s, e in phrases:
        span = e - s
        for k in range(s, e + 1):
            if k == s:
                pos[k] = "first"
            elif k == e:
                pos[k] = "last"
            else:
                r = (k - s) / span
                pos[k] = "early" if r < 0.4 else "middle" if r < 0.7 else "late"
    on, off8 = [], []
    for i, (t, d, bar, beat, tat, div, bd, num) in enumerate(notes):
        if (bar, beat) not in idx or not div:
            continue
        bi = idx[(bar, beat)]
        b0 = beats[bi][2]
        b1 = beats[bi + 1][2] if bi + 1 < len(beats) else b0 + bd
        blen = b1 - b0
        if blen <= 0:
            continue
        frac = Fraction(tat - 1, div)
        if frac == 0:
            dev = t - b0
            if abs(dev) <= blen / 8:
                on.append(dict(ms=1000 * dev, t=t, bi=bi, beat=beat, num=num, ph=pos.get(i),
                               chorus=beats[bi][3], blen=blen))
        elif frac in (Fraction(1, 2), Fraction(2, 3)):
            f = (t - b0) / blen
            if 0.35 < f < 0.85:
                off8.append(dict(f=f, ms_trip=1000 * (t - (b0 + 2 * blen / 3))))
    return beats, on, off8


SEL = [
    ("Louis Armstrong 1925-28", lambda p, y: p == "Louis Armstrong"),
    ("Charlie Parker", lambda p, y: p == "Charlie Parker"),
    ("Miles Davis 1951-59", lambda p, y: p == "Miles Davis" and y and 1951 <= y <= 1959),
    ("Sonny Rollins 1954-72", lambda p, y: p == "Sonny Rollins" and y and 1954 <= y <= 1972),
    ("John Coltrane 1960-64", lambda p, y: p == "John Coltrane" and y and 1960 <= y <= 1964),
    ("Wayne Shorter 1961-66", lambda p, y: p == "Wayne Shorter" and y and 1961 <= y <= 1966),
    ("whole database", lambda p, y: True),
]
rows = list(c.execute("select s.melid,s.performer,t.recordingdate,s.avgtempo from solo_info s join track_info t using(trackid)"))
data = {}
for melid, perf, rd, tempo in rows:
    beats, on, off8 = load(melid)
    if len(on) >= 20 and len(beats) >= 16:
        data[melid] = dict(perf=perf, year=year_of(rd), tempo=tempo or 0, beats=beats, on=on, off8=off8)


def group(f):
    return [d for d in data.values() if f(d["perf"], d["year"])]


def md(xs, f="%+.0f"):
    return (f + "/" + f) % (st.median(xs), st.mean(xs)) if xs else "n/a"


P("SOLOIST TIMING -- Weimar Jazz Database (wjazzd.db), statistics only")
P("=" * 74)
P("Reference = the beats table (hand-annotated band beat onsets). Lay-back = note onset")
P("minus the beat onset for notes notated on a beat and within +-1/8 beat of it;")
P("+ = behind (late), - = ahead. Values are 'median/mean' in ms unless noted.")
P("Caveat: the whole database sits ~+16 ms behind on average, so part of every number")
P("can be an annotation convention (beat taps vs note-onset marking); compare players")
P("against the 'whole database' row rather than against zero.")
P("")
for name, f in SEL:
    g = group(f)
    on = [x for d in g for x in d["on"]]
    P("#" * 74)
    P("%s   (%d solos, %d on-beat notes, tempo %s bpm)" % (
        name, len(g), len(on), "%.0f-%.0f" % (min(d["tempo"] for d in g), max(d["tempo"] for d in g))))
    # 1
    P("1. lay-back by phrase position (median/mean ms, n):")
    P("   " + "   ".join("%s %s (n=%d)" % (k, md([x["ms"] for x in on if x["ph"] == k]),
                                           sum(x["ph"] == k for x in on))
                          for k in ("first", "early", "middle", "late", "last")))
    P("   all on-beat notes: %s" % md([x["ms"] for x in on]))
    # 2
    o8 = [(x, d["tempo"]) for d in g for x in d["off8"]]
    def sw(xs):
        if not xs:
            return "n/a"
        fm = st.median(x["f"] for x in xs)
        return "lands at %.3f of the beat, %+.0f ms vs the triplet spot (median), swing ratio %.2f:1 (n=%d)" % (
            fm, st.median(x["ms_trip"] for x in xs), fm / (1 - fm), len(xs))
    P("2. offbeat 8ths (notes notated on the 'and' or the last triplet, 0.35-0.85 of the beat):")
    P("   overall: " + sw([x for x, _ in o8]))
    for lo, hi, lab in ((0, 140, "<140"), (140, 200, "140-200"), (200, 260, "200-260"), (260, 999, ">260")):
        P("   %-8s %s" % (lab + ":", sw([x for x, t in o8 if lo <= t < hi])))
    # 3
    slow, fast = [], []
    for d in g:
        xs = d["on"]
        for x in xs:
            w = [y["ms"] for y in xs if abs(y["bi"] - x["bi"]) <= 8]  # +-2 bars = 4-bar window
            rm = st.mean(w)
            slow.append(rm)
            fast.append(x["ms"] - rm)
    P("3. jitter: total SD %.1f ms = slow part (SD of the 4-bar running mean) %.1f ms + fast per-note part %.1f ms" % (
        st.pstdev([x["ms"] for x in on]), st.pstdev(slow), st.pstdev(fast)))
    # 4
    ts = [d["tempo"] for d in g]
    ms = [st.median(x["ms"] for x in d["on"]) for d in g]
    P("4. lay-back vs tempo: %+.1f ms per 10 bpm (per-solo medians; %d solos, %.0f-%.0f bpm)" % (
        10 * linfit(ts, ms), len(g), min(ts), max(ts)))
    # 5
    P("5. by beat of the bar (4/4 only): " + "   ".join(
        "beat %d %s" % (b, md([x["ms"] for x in on if x["num"] == 4 and x["beat"] == b])) for b in (1, 2, 3, 4)))
    # 6
    fc, lc, ratio = [], [], []
    for d in g:
        ch = sorted({x["chorus"] for x in d["on"] if x["chorus"] is not None and x["chorus"] > 0})
        if len(ch) >= 2:
            fc += [x["ms"] for x in d["on"] if x["chorus"] == ch[0]]
            lc += [x["ms"] for x in d["on"] if x["chorus"] == ch[-1]]
        # soloist's own tempo: fit note onsets against the beat index, compare to the band's fit
        bis = [x["bi"] for x in d["on"]]
        if max(bis) - min(bis) >= 16:
            sp = linfit(bis, [x["t"] for x in d["on"]])
            bb = [i for i in range(min(bis), max(bis) + 1)]
            bp = linfit(bb, [d["beats"][i][2] for i in bb])
            if sp > 0 and bp > 0:
                ratio.append(100 * (bp / sp - 1))
    P("6. drift: first chorus %s, last chorus %s (solos with >= 2 choruses);" % (md(fc), md(lc)))
    P("   soloist's own tempo vs the band's over the solo: median %+.2f%% (IQR %+.2f..%+.2f; + = soloist faster)" % (
        st.median(ratio), *(st.quantiles(ratio, n=4)[0::2])))
    P("")

# personal tendency
by_p = defaultdict(list)
for d in data.values():
    by_p[d["perf"]].append(st.median(x["ms"] for x in d["on"]))
big = {p: v for p, v in by_p.items() if len(v) >= 4}
allv = [x for v in big.values() for x in v]
gm = st.mean(allv)
eta = sum(len(v) * (st.mean(v) - gm) ** 2 for v in big.values()) / sum((x - gm) ** 2 for x in allv)
P("#" * 74)
P("IS LAY-BACK PERSONAL? %d performers with >= 4 solos (%d solos): who is playing explains" % (len(big), len(allv)))
P("%.0f%% of the solo-to-solo variation in median lay-back (eta squared)." % (100 * eta))
rank = sorted(big.items(), key=lambda kv: -st.median(kv[1]))
P("most behind:  " + "; ".join("%s %+.0f (%d)" % (p, st.median(v), len(v)) for p, v in rank[:6]))
P("most on top:  " + "; ".join("%s %+.0f (%d)" % (p, st.median(v), len(v)) for p, v in rank[-6:]))
print("\n".join(L))
