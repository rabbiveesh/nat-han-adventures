import sqlite3, re, sys, statistics as st
from collections import defaultdict
from fractions import Fraction
c = sqlite3.connect(sys.argv[1])
def year(s):
    m = re.findall(r"(19\d\d|20\d\d)", s or ""); return int(m[-1]) if m else None
solos = {}
for melid, perf, title, rd, tempo, feel, style, instr in c.execute(
        "select s.melid,s.performer,s.title,t.recordingdate,s.avgtempo,s.rhythmfeel,s.style,s.instrument from solo_info s join track_info t using(trackid)"):
    solos[melid] = dict(perf=perf, title=title, year=year(rd), tempo=tempo or 0, feel=feel or "", style=style, instr=instr)
print("PART 1: FUSION / POSTBOP SOLOS FROM 1968 ON")
rows = sorted((s for s in solos.values() if s["style"] in ("FUSION", "POSTBOP") and s["year"] and s["year"] >= 1968),
              key=lambda s: (s["feel"], s["perf"], s["year"]))
print("%-20s %-32s %4s %5s %-16s %-5s %s" % ("performer", "title", "year", "bpm", "rhythmfeel", "instr", "style"))
for s in rows:
    print("%-20s %-32s %4d %5.0f %-16s %-5s %s" % (s["perf"][:20], s["title"][:32], s["year"], s["tempo"], s["feel"], s["instr"], s["style"]))
print("(%d solos)" % len(rows))
names = ["Michael Brecker", "Randy Brecker", "Herbie Hancock", "Wayne Shorter", "David Sanborn", "Joe Henderson",
         "Jaco", "Chick Corea", "Pat Metheny", "Bob Berg", "Kenny Garrett", "Chris Potter", "Joe Zawinul", "John Scofield"]
print("\nnamed players: " + "; ".join("%s %d (%s)" % (n, sum(n in s["perf"] for s in solos.values()),
      ", ".join(sorted({"%s %s" % (s["feel"], s["year"]) for s in solos.values() if n in s["perf"]})) or "-") for n in names))

# timing per solo
def timing(melid):
    beats = list(c.execute("select bar,beat,onset from beats where melid=? order by onset", (melid,)))
    idx = {(b[0], b[1]): i for i, b in enumerate(beats)}
    on, sw, six = [], [], {1: [], 2: [], 3: []}
    for t, bar, beat, tat, div in c.execute("select onset,bar,beat,tatum,division from melody where melid=?", (melid,)):
        if (bar, beat) not in idx or not div: continue
        i = idx[(bar, beat)]
        if i + 1 >= len(beats): continue
        b0, b1 = beats[i][2], beats[i + 1][2]
        L = b1 - b0
        if L <= 0: continue
        fr = Fraction(tat - 1, div)
        x = (t - b0) / L
        if fr == 0 and abs(t - b0) <= L / 8: on.append(1000 * (t - b0))
        if fr in (Fraction(1, 2), Fraction(2, 3)) and 0.35 < x < 0.85: sw.append(x)
        if div == 4 and tat in (2, 3, 4) and abs(x - (tat - 1) / 4) < 0.15: six[tat - 1].append(x)
    return on, sw, six
by = defaultdict(list)
for melid, s in solos.items():
    on, sw, six = timing(melid)
    if len(on) >= 10:
        s.update(on=on, sw=sw, six=six); by[s["feel"]].append(s)
print("\nPART 1b: TIMING BY RHYTHM FEEL (all eras)")
print("lay-back = note on a beat minus the annotated beat (ms, within 1/8 beat); swing = where 8th offbeats land")
print("(0.50 straight, 0.67 triplet) and the long:short ratio; 16ths = where notated 16ths land within the beat")
print("(e/&/a ideal 0.25/0.50/0.75), median.")
def m(x, f="%+.0f"): return f % st.median(x) if x else "n/a"
for feel, ss in sorted(by.items(), key=lambda kv: -len(kv[1])):
    on = [x for s in ss for x in s["on"]]; sw = [x for s in ss for x in s["sw"]]
    e = [x for s in ss for x in s["six"][1]]; a = [x for s in ss for x in s["six"][2]]; n = [x for s in ss for x in s["six"][3]]
    swm = st.median(sw) if sw else None
    print("%-16s %3d solos, %5.0f bpm median | lay-back %s ms (per-solo median %s) | 8ths at %s = %s:1 | 16ths e %s, & %s, a %s (n=%d)" % (
        feel, len(ss), st.median(s["tempo"] for s in ss), m(on), m([st.median(s["on"]) for s in ss]),
        "%.3f" % swm if swm else "n/a", "%.2f" % (swm / (1 - swm)) if swm else "n/a",
        m(e, "%.3f"), m(a, "%.3f"), m(n, "%.3f"), len(e) + len(a) + len(n)))
for feel in ("FUNK", "LATIN"):
    print("\n%s solos, one line each:" % feel)
    for s in sorted(by.get(feel, []), key=lambda s: s["perf"]):
        sw = s["sw"]; swm = st.median(sw) if sw else None
        six = s["six"]
        print("  %-18s %-28s %4d %4.0f bpm  lay-back %s ms  8ths %s  16ths e/&/a %s/%s/%s" % (
            s["perf"][:18], s["title"][:28], s["year"] or 0, s["tempo"], m(s["on"]),
            ("%.2f:1" % (swm / (1 - swm))) if swm else "n/a", m(six[1], "%.2f"), m(six[2], "%.2f"), m(six[3], "%.2f")))
