import sqlite3, re, sys
from collections import Counter, defaultdict
c = sqlite3.connect(sys.argv[1])
PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
def root(sym):
    m = re.match(r"([A-G])([b#]?)", sym or "")
    return None if not m or sym == "NC" else (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12
def basses(lineup):
    out = []
    for part in (lineup or "").split(";"):
        m = re.match(r"\s*(.+?)\s*\((b|bass|db)\)\s*$", part.strip())
        if m:
            n = m.group(1).strip().title().replace("Curley", "Curly").replace("Dougwatkins", "Doug Watkins")
            out.append(re.sub(r"Russel$", "Russell", n))
    return out
by = defaultdict(list)
for melid, feel, sig, lineup in c.execute("select s.melid,s.rhythmfeel,s.signature,t.lineup from solo_info s join track_info t using(trackid)"):
    if "SWING" in (feel or "") and (sig or "4/4") == "4/4":
        for b in basses(lineup):
            by[b].append(melid)
        by["(whole DB)"].append(melid)
rows = []
for b, ms in by.items():
    C = Counter()
    for m in ms:
        bt = list(c.execute("select bar,beat,chord,bass_pitch from beats where melid=? order by onset", (m,)))
        ps = [x[3] if x[3] and x[3] > 0 else None for x in bt]
        cur = None
        roots = []
        for x in bt:
            r = root(x[2]) if x[2] else None
            chg = r is not None and r != cur
            if r is not None: cur = r
            roots.append((cur, chg))
        for i in range(1, len(bt)):
            if ps[i] is None or ps[i-1] is None: continue
            a = abs(ps[i] - ps[i-1]); C["iv"] += 1
            C["leap5"] += a >= 5; C["leap8"] += a >= 8; C["oct"] += a == 12
            # anticipation: the beat before a change already plays the new root; delay: the change beat holds the old note
            if roots[i][1] and roots[i][0] is not None:
                C["chg"] += 1
                C["antic"] += ps[i-1] % 12 == roots[i][0]
                C["delay"] += ps[i] % 12 == ps[i-1] % 12 and ps[i] % 12 != roots[i][0]
        # two-feel bars: beats 1=2 and 3=4 in pitch
        bars = defaultdict(dict)
        for x, p in zip(bt, ps):
            if p: bars[x[0]][x[1]] = p
        for bar, d in bars.items():
            if all(k in d for k in (1, 2, 3, 4)):
                C["bars"] += 1
                C["two"] += d[1] == d[2] and d[3] == d[4]
        # repeating cells: period 3 (hemiola-like, across the bar) vs period 4 (bar riff) over 6/8 beats
        for i in range(len(ps) - 8):
            w = ps[i:i + 8]
            if None in w or len({p % 12 for p in w}) < 2: continue
            C["win"] += 1
            if all(w[k] == w[k + 3] for k in range(5)) and len(set(w[:3])) > 1: C["p3"] += 1
            elif all(w[k] == w[k + 4] for k in range(4)) and len(set(w[:4])) > 1: C["p4"] += 1
            elif all(w[k] == w[k + 2] for k in range(6)): C["p2"] += 1
    if C["iv"] >= 500:
        rows.append((b, len(ms), C))
rows.sort(key=lambda r: -r[2]["iv"])
print("%-22s %5s %6s | %6s %6s %6s | %6s %6s | %6s | %6s %6s %6s" % ("bassist", "solos", "beats", "leap5+", "leap8+", "octave", "antic", "delay", "2-feel", "3-cell", "bar", "2-cell"))
for b, n, C in rows:
    p = lambda k, d: 100 * C[k] / max(1, C[d])
    print("%-22s %5d %6d | %5.1f%% %5.1f%% %5.1f%% | %5.1f%% %5.1f%% | %5.1f%% | %5.2f%% %5.2f%% %5.2f%%" % (
        b[:22], n, C["iv"], p("leap5", "iv"), p("leap8", "iv"), p("oct", "iv"), p("antic", "chg"), p("delay", "chg"),
        p("two", "bars"), p("p3", "win"), p("p4", "win"), p("p2", "win")))
