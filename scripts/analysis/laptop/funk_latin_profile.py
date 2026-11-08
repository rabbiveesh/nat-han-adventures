import sqlite3, re, sys, itertools, statistics as st
from collections import Counter, defaultdict
from fractions import Fraction

c = sqlite3.connect(sys.argv[1])
PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
NAMES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
L = []
P = L.append


def year_of(s):
    m = re.findall(r"(19\d\d|20\d\d)", s or "")
    return int(m[-1]) if m else None


def parse_chord(sym):
    if not sym or sym == "NC":
        return None
    m = re.match(r"([A-G])([b#]?)(.*)", sym.split("/")[0])
    if not m:
        return None
    root = (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12
    q = m.group(3)
    for pre, k in (("m7b5", "half"), ("o", "dim"), ("-", "min"), ("+", "dom"), ("sus", "dom"), ("j7", "maj"), ("7", "dom"), ("6", "maj")):
        if q.startswith(pre):
            return root, k
    return root, "maj"


def tones(ch):
    k = ch[1]
    third = 3 if k in ("min", "half", "dim") else 4
    fifth = 6 if k in ("half", "dim") else 7
    sev = {"maj": {9, 11}, "dim": {9}, "half": {10}, "min": {9, 10, 11}}.get(k, {10})
    return {0, third, fifth} | sev


DEG = {0: "1", 1: "b9", 2: "9", 4: "3", 7: "5", 10: "b7", 11: "7"}


def degree(r, ch):
    k = ch[1] if ch else None
    if r == 5:
        return "11"
    if r in DEG:
        return DEG[r]
    if r == 3:
        return "b3" if k in ("min", "half", "dim") else "#9"
    if r == 6:
        return "b5" if k in ("half", "dim") else "#11"
    if r == 8:
        return "b13"
    return "6" if k in ("maj", "min") else "13"


PENTAS = [frozenset((r + x) % 12 for x in (0, 2, 4, 7, 9)) for r in range(12)]


def load(melid):
    beats = list(c.execute("select bar,beat,onset from beats where melid=? order by onset", (melid,)))
    bidx = {(b[0], b[1]): i for i, b in enumerate(beats)}
    bpb = defaultdict(int)
    for bar, beat, _ in beats:
        bpb[bar] = max(bpb[bar], beat)
    start, acc = {}, 0
    for b in sorted(bpb):
        start[b] = acc
        acc += bpb[b]
    notes = []
    for (on, p, d, bar, beat, tat, div, num, bd, lm, f0) in c.execute(
            "select onset,pitch,duration,bar,beat,tatum,division,num,beatdur,loud_med,f0_mod from melody where melid=? order by eventid", (melid,)):
        frac = Fraction(tat - 1, div) if div else Fraction(0)
        bi = bidx.get((bar, beat))
        dev = None
        if bi is not None and bi + 1 < len(beats):
            b0, b1 = beats[bi][2], beats[bi + 1][2]
            dev = (on - b0, (on - b0) / (b1 - b0) if b1 > b0 else None, b1 - b0)
        notes.append(dict(on=on, p=int(round(p)), d=d, bar=bar, beat=beat, frac=frac, div=div,
                          pos=start.get(bar, bar * (num or 4)) + (beat - 1) + frac, bd=bd or 0.5, num=num or 4,
                          loud=lm if lm is not None and lm > 20 else None, f0=f0 or "", dev=dev))
    chords, csym = [None] * len(notes), [None] * len(notes)
    for s, e, v in c.execute("select start,end,value from sections where melid=? and type='CHORD'", (melid,)):
        for k in range(s, min(e, len(notes) - 1) + 1):
            chords[k], csym[k] = parse_chord(v), v
    phrases = [(s, min(e, len(notes) - 1)) for s, e in
               c.execute("select start,end from sections where melid=? and type='PHRASE' order by start", (melid,))]
    return notes, chords, csym, phrases


def q(xs, f="%.2f"):
    if not xs:
        return "n/a"
    qq = st.quantiles(xs, n=4)
    return (f + " " + f + " " + f) % (qq[0], st.median(xs), qq[2])


def profile(label, solos):
    D = defaultdict(list)
    C = Counter()
    data = {}
    for s in solos:
        n, ch, cs, ph = load(s["melid"])
        data[s["melid"]] = (n, ch, cs, ph)
        if not n:
            continue
        louds = [x["loud"] for x in n if x["loud"] is not None]
        ml = st.median(louds) if louds else 0
        mp = st.median(x["p"] for x in n)
        for x in n:
            x["db"] = None if x["loud"] is None else x["loud"] - ml
            D["pitch"].append(x["p"])
            if x["db"] is not None:
                D["db"].append(x["db"]); D["octx"].append((x["p"] - mp) / 12)
                if x["d"] / x["bd"] >= 1.2:
                    D["longdb"].append(x["db"])
            C["notes"] += 1
            C["fall"] += x["f0"] in ("fall-off", "slide", "bend")
            C["fall_only"] += x["f0"] == "fall-off"
            if x["dev"] and x["frac"] == 0 and abs(x["dev"][0]) <= x["dev"][2] / 8:
                D["lay"].append(1000 * x["dev"][0])
                D["lay_t"].append((s["tempo"], 1000 * x["dev"][0]))
            if x["dev"] and x["dev"][1] is not None:
                f = x["dev"][1]
                if x["frac"] in (Fraction(1, 2), Fraction(2, 3)) and 0.35 < f < 0.85:
                    D["sw8"].append(f)
                if x["div"] == 4 and x["frac"] == Fraction(1, 4) and 0.1 < f < 0.42:
                    D["sw16e"].append(f)
                if x["div"] == 4 and x["frac"] == Fraction(3, 4) and 0.6 < f < 0.92:
                    D["sw16a"].append(f)
        for pi, (a, b) in enumerate(ph):
            seg = n[a:b + 1]
            if not seg:
                continue
            C["phr"] += 1
            D["ph_beats"].append(float(seg[-1]["pos"] - seg[0]["pos"]) + seg[-1]["d"] / seg[-1]["bd"])
            if pi + 1 < len(ph) and ph[pi + 1][0] < len(n):
                D["rest"].append((n[ph[pi + 1][0]]["on"] - seg[-1]["on"] - seg[-1]["d"]) / seg[-1]["bd"])
            C["ant"] += seg[0]["frac"] == Fraction(1, 2)
            if ch[b]:
                C["endn"] += 1; C["endct"] += (seg[-1]["p"] - ch[b][0]) % 12 in tones(ch[b])
            if len(seg) >= 5 and all(x["db"] is not None for x in seg):
                D["a0"].append(seg[0]["db"]); D["a4"].append(seg[-1]["db"])
                mid = seg[1:-1]; k = len(mid)
                D["a1"] += [x["db"] for x in mid[:k // 3]]
                D["a2"] += [x["db"] for x in mid[k // 3:2 * k // 3]]
                D["a3"] += [x["db"] for x in mid[2 * k // 3:]]
            for i in range(len(seg) - 1):
                ioi = seg[i + 1]["pos"] - seg[i]["pos"]
                cell = {Fraction(1, 2): "eighths", Fraction(1): "quarter", Fraction(1, 3): "triplet",
                        Fraction(1, 4): "sixteenths", Fraction(3, 4): "dotted", Fraction(3, 2): "dotted",
                        Fraction(2, 3): "quarter_triplet"}.get(ioi)
                if cell is None and ioi > 1:
                    cell = "long"
                if cell:
                    C["cell_" + cell] += 1; C["cells"] += 1
                r = seg[i + 1]["on"] - seg[i]["on"]
                if r > 0:
                    D["legato"].append(min(1.5, seg[i]["d"] / r))
                iv = seg[i + 1]["p"] - seg[i]["p"]
                C["iv"] += 1
                C["iv_%s" % (abs(iv) if abs(iv) <= 7 else "8-12" if abs(iv) <= 12 else "13+")] += 1
                if abs(iv) >= 5 and i + 2 < len(seg):
                    iv2 = seg[i + 2]["p"] - seg[i + 1]["p"]
                    C["leap"] += 1; C["stepback"] += 1 <= abs(iv2) <= 2 and (iv2 > 0) != (iv > 0)
            for i in range(a + 1, b + 1):
                x = n[i]
                strong = x["frac"] == 0 and (x["beat"] in (1, 3) if x["num"] == 4 else x["beat"] == 1)
                if strong and ch[i] and (x["p"] - ch[i][0]) % 12 in tones(ch[i]):
                    C["tgt"] += 1
                    dp = n[i - 1]["p"] - x["p"]
                    C["app"] += abs(dp) == 1; C["below"] += dp == -1
                    if i - 2 >= a:
                        d2 = n[i - 2]["p"] - x["p"]
                        C["encl"] += 1 <= abs(dp) <= 2 and 1 <= abs(d2) <= 2 and (dp > 0) != (d2 > 0)
            for i in range(len(seg) - 3):
                C["w4"] += 1
                C["pent"] += any({x["p"] % 12 for x in seg[i:i + 4]} <= pp for pp in PENTAS)
            rep = None
            for i in range(len(seg) - 7):
                c1 = [seg[i + k + 1]["p"] - seg[i + k]["p"] for k in range(3)]
                c2 = [seg[i + 4 + k + 1]["p"] - seg[i + 4 + k]["p"] for k in range(3)]
                if c1 == c2 and any(c1):
                    rep = seg[i + 4]["p"] - seg[i]["p"]
                    break
            if rep is not None:
                if rep == 0:
                    C["motif"] += 1
                else:
                    C["seq"] += 1; D["moves"].append(rep)

    pct = lambda a, b: a / b if b else 0
    P("#" * 100)
    P("%s   (%d solos, %d notes, %d phrases; tempo %.0f-%.0f bpm, median %.0f)" % (
        label, len(solos), C["notes"], C["phr"], min(s["tempo"] for s in solos), max(s["tempo"] for s in solos),
        st.median(s["tempo"] for s in solos)))
    P("solos: " + "; ".join("%s %s (%s, %.0f)" % (s["perf"], s["title"], s["year"], s["tempo"]) for s in solos))
    P("#" * 100)
    P("[line]")
    P("phrase_beats      = %s    # Q1 median Q3" % q(D["ph_beats"], "%.1f"))
    P("rest_beats        = %s" % q(D["rest"], "%.2f"))
    cells = ["eighths", "quarter", "triplet", "sixteenths", "dotted", "quarter_triplet", "long"]
    P("rhythm_cells      = " + " ".join("%s:%.3f" % (k, pct(C["cell_" + k], C["cells"])) for k in cells))
    ivk = [str(i) for i in range(8)] + ["8-12", "13+"]
    P("intervals         = " + " ".join("%s:%.3f" % (k, pct(C["iv_" + k], C["iv"])) for k in ivk))
    P("step_back         = %.3f    # leaps (>=5) answered by a 1-2 step the other way" % pct(C["stepback"], C["leap"]))
    P("end_chord_tone    = %.3f" % pct(C["endct"], C["endn"]))
    P("approach          = %.3f    # strong-beat chord tones approached by a half step" % pct(C["app"], C["tgt"]))
    P("from_below        = %.3f    # share of those approaches from below" % pct(C["below"], C["app"]))
    P("enclosure         = %.3f" % pct(C["encl"], C["tgt"]))
    P("pentatonic        = %.3f    # 4-note windows inside one pentatonic" % pct(C["pent"], C["w4"]))
    P("anticipation      = %.3f    # phrases starting on an 'and'" % pct(C["ant"], C["phr"]))
    P("falls             = %.3f    # notes marked fall-off/slide/bend (fall-off alone %.3f)" % (pct(C["fall"], C["notes"]), pct(C["fall_only"], C["notes"])))
    pp = sorted(D["pitch"])
    P("register          = centre %d  floor %d  ceiling %d   # concert MIDI: median, 5th, 95th percentile" % (
        st.median(pp), pp[len(pp) // 20], pp[len(pp) * 19 // 20]))
    P("legato            = %.2f    # median sounding duration / inter-onset interval" % st.median(D["legato"]))
    P("[dynamics]   # loud_med per note, dB relative to the solo's median")
    P("arch              = %+.2f %+.2f %+.2f %+.2f %+.2f   # first early middle late last" % tuple(
        st.mean(D[k]) for k in ("a0", "a1", "a2", "a3", "a4")))
    xs, ys = D["octx"], D["db"]
    mx, my = st.mean(xs), st.mean(ys)
    slope = sum((a - mx) * (b - my) for a, b in zip(xs, ys)) / sum((a - mx) ** 2 for a in xs)
    P("per_octave        = %+.2f" % slope)
    P("                    # (%d of %d notes have no loudness in the DB: its -66 dB 'failed' value; left out)" % (
        C["notes"] - len(ys), C["notes"]))
    P("ghost             = %.3f    # notes < -3 dB" % (sum(y < -3 for y in ys) / len(ys)))
    P("accent            = %.3f    # notes > +3 dB" % (sum(y > 3 for y in ys) / len(ys)))
    P("long              = %+.2f   # notes >= 1.2 beats (n=%d)" % (st.mean(D["longdb"]) if D["longdb"] else 0, len(D["longdb"])))
    P("scatter           = %.2f    # SD of note dB" % st.pstdev(ys))
    P("[motifs]")
    P("motif             = %.3f    # phrases with a 4-note cell repeated at once, same pitch" % pct(C["motif"], C["phr"]))
    P("sequence          = %.3f    # ... repeated transposed" % pct(C["seq"], C["phr"]))
    mv = Counter(D["moves"])
    P("moves             = " + (" ".join("%+d:%d" % kv for kv in sorted(mv.items(), key=lambda kv: (-kv[1], kv[0]))) or "none"))
    P("[feel]")
    sw8 = st.median(D["sw8"]) if D["sw8"] else None
    e16 = st.median(D["sw16e"]) if D["sw16e"] else None
    a16 = st.median(D["sw16a"]) if D["sw16a"] else None
    P("swing_8th         = %s   # offbeat 8th lands at %s of the beat (n=%d)" % (
        "%.2f" % (sw8 / (1 - sw8)) if sw8 else "n/a", "%.3f" % sw8 if sw8 else "n/a", len(D["sw8"])))
    P("swing_16th        = %s   # long:short 16th from the 'e' (lands at %s, n=%d); from the 'a': %s (lands at %s, n=%d)" % (
        "%.2f" % (e16 / (0.5 - e16)) if e16 else "n/a", "%.3f" % e16 if e16 else "n/a", len(D["sw16e"]),
        "%.2f" % ((a16 - 0.5) / (1 - a16)) if a16 else "n/a", "%.3f" % a16 if a16 else "n/a", len(D["sw16a"])))
    P("lay_back_ms       = median %+.1f mean %+.1f sd %.1f  (n=%d notes on a beat, within 1/8 beat)" % (
        st.median(D["lay"]), st.mean(D["lay"]), st.pstdev(D["lay"]), len(D["lay"])))
    for lo, hi in ((0, 100), (100, 130), (130, 160), (160, 200), (200, 400)):
        v = [m for t, m in D["lay_t"] if lo <= t < hi]
        if v:
            P("  lay_back %3d-%3d bpm = %+.1f ms median (n=%d)" % (lo, hi, st.median(v), len(v)))
    P("")
    licks(label, solos, data)


def licks(label, solos, data):
    occ = defaultdict(list)
    for s in solos:
        n, ch, cs, ph = data[s["melid"]]
        for a, b in ph:
            ps = [x["p"] for x in n[a:b + 1]]
            for Ln in range(4, 7):
                last = {}
                for i in range(len(ps) - Ln + 1):
                    iv = tuple(ps[i + k + 1] - ps[i + k] for k in range(Ln - 1))
                    if last.get(iv, -1) > i or not any(iv):
                        continue
                    last[iv] = i + Ln - 1
                    occ[iv].append((s["melid"], a + i))
    cnt = {k: len(v) for k, v in occ.items()}
    nsol = {k: len({m for m, _ in v}) for k, v in occ.items()}
    ext = defaultdict(int)
    for iv in occ:
        if len(iv) >= 4:
            ext[iv[1:]] = max(ext[iv[1:]], cnt[iv]); ext[iv[:-1]] = max(ext[iv[:-1]], cnt[iv])
    cands = [iv for iv in occ if nsol[iv] >= 3 and ext.get(iv, 0) < 0.8 * cnt[iv]
             and len(set(itertools.accumulate((0,) + iv))) >= 3]
    cands.sort(key=lambda iv: -(cnt[iv] * (len(iv) + 1)))
    kept, runs = [], 0
    def related(a, b):
        sa, sb = "," + ",".join(map(str, a)) + ",", "," + ",".join(map(str, b)) + ","
        return sa in sb or sb in sa
    for iv in cands:
        if any(related(iv, k) for k in kept):
            continue
        if all(1 <= x <= 2 for x in iv) or all(-2 <= x <= -1 for x in iv):
            if runs >= 2:
                continue
            runs += 1
        kept.append(iv)
        if len(kept) == 12:
            break
    P("LICK CELLS (%s): recurring 4-6 note interval patterns in >= 3 solos, ranked occurrences x length." % label)
    P("Example = the occurrence whose chord kind is the commonest, rooted on C (chord root at the first note")
    P("moved to C, original octave kept as near as possible); tag = chord kind on >= 60% of occurrences, else 'any'.")
    for r, iv in enumerate(kept, 1):
        kinds = Counter()
        for m, s0 in occ[iv]:
            ch = data[m][1][s0]
            kinds[ch[1] if ch else "none"] += 1
        top, topn = kinds.most_common(1)[0]
        tag = top if topn >= 0.6 * cnt[iv] and top not in ("none",) else "any"
        tag = {"min": "min", "maj": "maj", "dom": "dom", "half": "half", "dim": "any"}.get(tag, "any")
        m, s0 = next(((m, s0) for m, s0 in occ[iv] if data[m][1][s0] and data[m][1][s0][1] == top), occ[iv][0])
        n, ch, cs, ph = data[m]
        seg = n[s0:s0 + len(iv) + 1]
        c0 = ch[s0]
        shift = (-c0[0] if c0[0] <= 6 else 12 - c0[0]) if c0 else 0
        sx = next(s for s in solos if s["melid"] == m)
        P("")
        P("#%d [%s]  intervals [%s]  %d occurrences in %d solos   chord kinds: %s" % (
            r, tag, " ".join("%+d" % x for x in iv), cnt[iv], nsol[iv], ", ".join("%s %d" % kv for kv in kinds.most_common(4))))
        P("   on C: %s   degrees: %s   over %s" % (" ".join("%s%d" % (NAMES[(x["p"] + shift) % 12], (x["p"] + shift) // 12 - 1) for x in seg),
                                                 " ".join(degree((x["p"] - c0[0]) % 12, c0) if c0 else "?" for x in seg), cs[s0] or "NC"))
        P("   rhythm (beats from 1st note): %s   metric start %d%s   from %s, %s (%s)" % (
            " ".join(str(Fraction(x["pos"] - seg[0]["pos"]).limit_denominator(12)) for x in seg), seg[0]["beat"],
            {Fraction(0): "", Fraction(1, 4): "e", Fraction(1, 2): "&", Fraction(3, 4): "a"}.get(seg[0]["frac"], "+%s" % seg[0]["frac"]),
            sx["perf"], sx["title"], sx["year"]))
    P("")


solos = []
for melid, perf, title, rd, tempo, feel in c.execute(
        "select s.melid,s.performer,s.title,t.recordingdate,s.avgtempo,s.rhythmfeel from solo_info s join track_info t using(trackid)"):
    solos.append(dict(melid=melid, perf=perf, title=title, year=year_of(rd), tempo=tempo or 0, feel=feel or ""))
P("FUNK AND LATIN SOLOISTS -- Weimar Jazz Database (statistics; examples are study material only)")
P("=" * 100)
P("Groups by WJazzD rhythmfeel: FUNK (20 solos) and LATIN (27 solos); the 3 LATIN/FUNK solos and the mixed")
P("ones are left out. Same definitions as the swing-player reports.")
P("")
profile("FUNK", [s for s in solos if s["feel"] == "FUNK"])
profile("LATIN", [s for s in solos if s["feel"] == "LATIN"])
print("\n".join(L))
