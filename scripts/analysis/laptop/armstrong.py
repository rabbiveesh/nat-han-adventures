import sqlite3, re, sys, itertools, statistics as st
from collections import Counter, defaultdict
from fractions import Fraction

c = sqlite3.connect(sys.argv[1])
PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
NAMES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
MML = ["c", "d-", "d", "e-", "e", "f", "g-", "g", "a-", "a", "b-", "b"]
out_lines = []
P = out_lines.append


# ---------------------------------------------------------------- loading
def year_of(s):
    m = re.findall(r"(1[89]\d\d|20\d\d)", s or "")
    return int(m[-1]) if m else None


def parse_chord(sym):
    if not sym or sym == "NC":
        return None
    sym = sym.split("/")[0]
    m = re.match(r"([A-G])([b#]?)(.*)", sym)
    if not m:
        return None
    root = (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12
    q = m.group(3)
    for pre, k in (("m7b5", "hdim"), ("o", "dim"), ("-", "min"), ("+", "aug"), ("sus", "sus"), ("j7", "maj"), ("7", "dom"), ("6", "maj6")):
        if q.startswith(pre):
            kind = k
            break
    else:
        kind = "maj"
    if kind == "min" and ("6" in q[:2] or "j7" in q):
        kind = "mintonic"
    return root, kind


def chord_tones(ch):
    kind = ch[1]
    third = 3 if kind in ("min", "mintonic", "hdim", "dim") else (5 if kind == "sus" else 4)
    fifth = 6 if kind in ("hdim", "dim") else (8 if kind == "aug" else 7)
    sev = {"maj": {11, 9}, "maj6": {9, 11}, "mintonic": {9, 11}, "dim": {9}, "hdim": {10}}.get(kind, {10})
    return {0, third, fifth} | sev


def is_ct(p, ch):
    return ch is not None and ((p - ch[0]) % 12) in chord_tones(ch)


def load(melid):
    rows = list(c.execute(
        "select onset,pitch,duration,bar,beat,tatum,division,num,beatdur,loud_med,f0_mod from melody where melid=? order by eventid",
        (melid,)))
    bpb, beat_on = defaultdict(int), {}
    for bar, beat, on in c.execute("select bar,beat,onset from beats where melid=?", (melid,)):
        bpb[bar] = max(bpb[bar], beat)
        beat_on[(bar, beat)] = on
    start, acc = {}, 0
    for b in sorted(bpb):
        start[b] = acc
        acc += bpb[b]
    notes = []
    for (on, p, d, bar, beat, tat, div, num, bd, lm, f0) in rows:
        frac = Fraction(tat - 1, div) if div else Fraction(0)
        notes.append(dict(onset=on, pitch=int(round(p)), dur=d, bar=bar, beat=beat, frac=frac, div=div,
                          pos=start.get(bar, bar * (num or 4)) + (beat - 1) + frac, bd=bd, num=num or 4,
                          loud=lm, f0=f0 or ""))
    chords, csym, cidx = [None] * len(notes), [None] * len(notes), [None] * len(notes)
    secs = list(c.execute("select start,end,value from sections where melid=? and type='CHORD' order by start", (melid,)))
    for i, (s, e, v) in enumerate(secs):
        for k in range(s, min(e, len(notes) - 1) + 1):
            chords[k], csym[k], cidx[k] = parse_chord(v), v, i
    nextch = {}
    for i in range(len(secs)):
        for j in range(i + 1, len(secs)):
            if secs[j][2] != secs[i][2]:
                nextch[i] = (parse_chord(secs[j][2]), secs[j][2])
                break
    phrases = [(s, min(e, len(notes) - 1)) for s, e in
               c.execute("select start,end from sections where melid=? and type='PHRASE' order by start", (melid,))]
    ideas = [v for (v,) in c.execute("select value from sections where melid=? and type='IDEA'", (melid,))]
    return dict(notes=notes, chords=chords, csym=csym, cidx=cidx, nextch=nextch, phrases=phrases,
                ideas=ideas, beat_on=beat_on)


def solos_where(pred):
    out = []
    for melid, perf, title, rdate, key, tempo, instr in c.execute(
            "select s.melid,s.performer,s.title,t.recordingdate,s.key,s.avgtempo,s.instrument from solo_info s join track_info t using(trackid) order by melid"):
        y = year_of(rdate)
        if pred(perf, y):
            out.append(dict(melid=melid, perf=perf, title=title, year=y, key=key, tempo=tempo, instr=instr))
    return out


# ---------------------------------------------------------------- helpers
def pct(a, b):
    return "%.1f%%" % (100 * a / b) if b else "n/a"


def miqr(xs, f="%.2f"):
    if not xs:
        return "n/a"
    q = st.quantiles(xs, n=4) if len(xs) > 1 else [xs[0]] * 3
    return (f + " [" + f + "-" + f + "]") % (st.median(xs), q[0], q[2])


PENTAS = [frozenset((r + x) % 12 for x in (0, 2, 4, 7, 9)) for r in range(12)]


# ---------------------------------------------------------------- A/B/C stats
def stats(solos, label):
    S = defaultdict(list)
    C = Counter()
    instr = Counter()
    for s in solos:
        d = load(s["melid"])
        s["data"] = d
        n = d["notes"]
        instr[s["instr"]] += 1
        if not n:
            continue
        span = n[-1]["onset"] + n[-1]["dur"] - n[0]["onset"]
        beats = float(n[-1]["pos"] - n[0]["pos"]) + n[-1]["dur"] / n[-1]["bd"]
        C["notes"] += len(n); C["secs"] += span; C["beats"] += beats
        S["tempo"].append(s["tempo"] or 0)
        sounding = sum(min(n[i]["dur"], n[i + 1]["onset"] - n[i]["onset"]) for i in range(len(n) - 1)) + n[-1]["dur"]
        C["sound"] += sounding
        louds = [x["loud"] for x in n if x["loud"] is not None]
        med_loud = st.median(louds) if louds else 0
        for x in n:
            x["rdb"] = None if x["loud"] is None else x["loud"] - med_loud
        C["no_loud"] += len(n) - len(louds)
        med_pitch = st.median(x["pitch"] for x in n)
        for x in n:
            S["pitch"].append(x["pitch"])
            if x["rdb"] is not None:
                S["rdb"].append(x["rdb"])
                S["oct_x"].append((x["pitch"] - med_pitch) / 12)
            if x["f0"]:
                C["f0_" + x["f0"]] += 1
            if x["rdb"] is not None and x["dur"] / x["bd"] >= 1.2:
                S["longdb"].append(x["rdb"])
            if x["rdb"] is not None and x["rdb"] < -3:
                C["ghost"] += 1
            # on-beat timing
            if x["frac"] == 0 and (x["bar"], x["beat"]) in d["beat_on"]:
                S["ontime"].append(1000 * (x["onset"] - d["beat_on"][(x["bar"], x["beat"])]))
        for i, x in enumerate(n):
            ch = d["chords"][i]
            strong = x["frac"] == 0 and (x["beat"] in (1, 3) if x["num"] == 4 else x["beat"] == 1)
            if strong and ch:
                C["strong"] += 1
                if is_ct(x["pitch"], ch):
                    C["strong_ct"] += 1
        # swing pairs / offbeat louder
        for i in range(len(n) - 1):
            a, b = n[i], n[i + 1]
            if a["frac"] == 0 and b["frac"] == Fraction(1, 2) and a["bar"] == b["bar"] and a["beat"] == b["beat"]:
                nb = d["beat_on"].get((a["bar"], a["beat"] + 1)) or d["beat_on"].get((a["bar"] + 1, 1))
                if nb and nb > b["onset"]:
                    S["swing"].append((b["onset"] - a["onset"]) / (nb - b["onset"]))
                if a["rdb"] is not None and b["rdb"] is not None:
                    C["pairs"] += 1
                    C["off_louder"] += b["rdb"] > a["rdb"]
        ph = d["phrases"]
        C["phrases"] += len(ph)
        for pi, (a, b) in enumerate(ph):
            seg = n[a:b + 1]
            if not seg:
                continue
            S["ph_notes"].append(len(seg))
            S["ph_beats"].append(float(seg[-1]["pos"] - seg[0]["pos"]) + seg[-1]["dur"] / seg[-1]["bd"])
            if pi + 1 < len(ph) and ph[pi + 1][0] < len(n):
                nx = n[ph[pi + 1][0]]
                S["rest"].append((nx["onset"] - (seg[-1]["onset"] + seg[-1]["dur"])) / seg[-1]["bd"])
            C["ant"] += seg[0]["frac"] == Fraction(1, 2)
            C["offstart"] += seg[0]["frac"] != 0
            if d["chords"][b]:
                C["ph_end"] += 1
                C["ph_end_ct"] += is_ct(seg[-1]["pitch"], d["chords"][b])
            # arch
            if len(seg) >= 5 and all(x["rdb"] is not None for x in seg):
                S["arch_first"].append(seg[0]["rdb"]); S["arch_last"].append(seg[-1]["rdb"])
                mid = seg[1:-1]
                k = len(mid)
                for nm, part in (("arch_early", mid[:k // 3]), ("arch_mid", mid[k // 3:2 * k // 3]), ("arch_late", mid[2 * k // 3:])):
                    S[nm].extend(x["rdb"] for x in part)
            for i in range(len(seg) - 1):
                ioi = seg[i + 1]["pos"] - seg[i]["pos"]
                cat = {Fraction(1, 4): "16th", Fraction(1, 3): "8th-triplet", Fraction(1, 2): "8th",
                       Fraction(2, 3): "quarter-triplet", Fraction(3, 4): "dotted 8th", Fraction(1): "quarter"}.get(ioi)
                if cat is None:
                    cat = "faster than 16th" if ioi < Fraction(1, 4) else ("longer" if ioi > 1 else "other")
                C["ioi_" + cat] += 1; C["ioi"] += 1
                real_ioi = seg[i + 1]["onset"] - seg[i]["onset"]
                if real_ioi > 0:
                    S["legato"].append(seg[i]["dur"] / real_ioi)
                iv = seg[i + 1]["pitch"] - seg[i]["pitch"]
                C["iv"] += 1
                C["iv_%d" % iv] += 1
                a_ = abs(iv)
                C["rep" if a_ == 0 else "step" if a_ <= 2 else "third" if a_ <= 4 else "leap"] += 1
                if a_ >= 5 and i + 2 < len(seg):
                    iv2 = seg[i + 2]["pitch"] - seg[i + 1]["pitch"]
                    C["leap_n"] += 1
                    C["leap_back"] += 1 <= abs(iv2) <= 2 and (iv2 > 0) != (iv > 0)
            # approaches into strong-beat chord tones
            for i in range(a + 1, b + 1):
                x = n[i]
                ch = d["chords"][i]
                strong = x["frac"] == 0 and (x["beat"] in (1, 3) if x["num"] == 4 else x["beat"] == 1)
                if strong and ch and is_ct(x["pitch"], ch):
                    C["tgt"] += 1
                    dp = n[i - 1]["pitch"] - x["pitch"]
                    C["app_below"] += dp == -1
                    C["app_above"] += dp == 1
                    if i - 2 >= a:
                        d2 = n[i - 2]["pitch"] - x["pitch"]
                        C["encl"] += (1 <= abs(dp) <= 2 and 1 <= abs(d2) <= 2 and (dp > 0) != (d2 > 0))
            for i in range(len(seg) - 3):
                C["win4"] += 1
                pcs = {x["pitch"] % 12 for x in seg[i:i + 4]}
                C["pent"] += any(pcs <= P5 for P5 in PENTAS)
            # immediate restatement of a 4-note cell
            found = None
            for i in range(len(seg) - 7):
                c1 = [seg[i + k + 1]["pitch"] - seg[i + k]["pitch"] for k in range(3)]
                c2 = [seg[i + 4 + k + 1]["pitch"] - seg[i + 4 + k]["pitch"] for k in range(3)]
                if c1 == c2 and any(c1):
                    found = seg[i + 4]["pitch"] - seg[i]["pitch"]
                    break
            if found is not None:
                C["restate"] += 1
                S["restate_t"].append(found)
            C["ph_counted"] += 1
        C["ideas"] += len(d["ideas"])
        C["ideas_var"] += sum("#" in v for v in d["ideas"])

    P("== %s ==  (%d solos; instruments: %s)" % (label, len(solos), ", ".join("%s x%d" % kv for kv in instr.most_common())))
    P("A. LINE")
    P("  notes per 4-beat bar: %.2f   notes per second: %.2f   avg tempo: %.0f bpm (solos: %s)" % (
        4 * C["notes"] / C["beats"], C["notes"] / C["secs"], st.mean(S["tempo"]), miqr(S["tempo"], "%.0f")))
    P("  silent share of solo time: %s" % pct(C["secs"] - C["sound"], C["secs"]))
    P("  phrases: %d; length in notes %s; length in beats %s" % (C["phrases"], miqr(S["ph_notes"], "%.0f"), miqr(S["ph_beats"], "%.1f")))
    P("  rest before next phrase (beats): %s" % miqr(S["rest"], "%.2f"))
    P("  phrase starts on the 'and' (8th anticipation): %s   (any off-beat start: %s)" % (
        pct(C["ant"], C["ph_counted"]), pct(C["offstart"], C["ph_counted"])))
    cats = ["faster than 16th", "16th", "8th-triplet", "8th", "quarter-triplet", "dotted 8th", "quarter", "longer", "other"]
    P("  IOIs within phrases (metrical grid): " + ", ".join("%s %s" % (k, pct(C["ioi_" + k], C["ioi"])) for k in cats))
    P("  intervals within phrases (signed semitones, % of all):")
    P("    " + "  ".join("%+d:%.1f" % (k, 100 * C["iv_%d" % k] / C["iv"]) for k in range(-12, 0)))
    P("    " + "  ".join("%+d:%.1f" % (k, 100 * C["iv_%d" % k] / C["iv"]) for k in range(0, 13)))
    outside = C["iv"] - sum(C["iv_%d" % k] for k in range(-12, 13))
    P("    beyond an octave: %s" % pct(outside, C["iv"]))
    P("  repeats %s, steps(1-2) %s, thirds(3-4) %s, leaps(>=5) %s; leaps followed by a 1-2 step back: %s" % (
        pct(C["rep"], C["iv"]), pct(C["step"], C["iv"]), pct(C["third"], C["iv"]), pct(C["leap"], C["iv"]),
        pct(C["leap_back"], C["leap_n"])))
    P("  chord tones (1 3 5 7/6) on beats 1 and 3: %s   phrases ending on a chord tone: %s" % (
        pct(C["strong_ct"], C["strong"]), pct(C["ph_end_ct"], C["ph_end"])))
    P("  into strong-beat chord tones (n=%d): half-step from below %s, from above %s, enclosure %s" % (
        C["tgt"], pct(C["app_below"], C["tgt"]), pct(C["app_above"], C["tgt"]), pct(C["encl"], C["tgt"])))
    P("  4-note windows inside one pentatonic: %s" % pct(C["pent"], C["win4"]))
    P("  register (MIDI): median %s, min %d (%s), max %d (%s)" % (
        miqr(S["pitch"], "%.0f"), min(S["pitch"]), nname(min(S["pitch"])), max(S["pitch"]), nname(max(S["pitch"]))))
    P("  swing ratio (metrical 8th pairs, n=%d): median %s" % (len(S["swing"]), miqr(S["swing"], "%.2f")))
    if S["ontime"]:
        P("  on-beat notes vs the beat grid (n=%d): mean %+.0f ms, SD %.0f ms, median %+.0f ms (negative = ahead)" % (
            len(S["ontime"]), st.mean(S["ontime"]), st.pstdev(S["ontime"]), st.median(S["ontime"])))
    P("  legato (sounding duration / IOI): median %s" % miqr(S["legato"], "%.2f"))
    P("  f0 modulation share of notes: " + ", ".join("%s %s" % (k, pct(C["f0_" + k], C["notes"])) for k in ("vibrato", "fall-off", "slide", "bend")))
    P("B. DYNAMICS (loud_med per note, dB relative to the solo's median)")
    P("  phrase arch (phrases >= 5 notes, mean dB): first %+.2f, early %+.2f, middle %+.2f, late %+.2f, last %+.2f" % tuple(
        st.mean(S[k]) for k in ("arch_first", "arch_early", "arch_mid", "arch_late", "arch_last")))
    xs, ys = S["oct_x"], S["rdb"]
    mx, my = st.mean(xs), st.mean(ys)
    slope = sum((a - mx) * (b - my) for a, b in zip(xs, ys)) / sum((a - mx) ** 2 for a in xs)
    P("  dB per octave above the solo's median pitch (regression slope): %+.2f" % slope)
    P("  offbeat 8th louder than its beat (n=%d pairs): %s" % (C["pairs"], pct(C["off_louder"], C["pairs"])))
    P("  notes without a loudness value (left out): %d" % C["no_loud"])
    P("  ghosted notes (< -3 dB): %s   long notes (>= 1.2 beats, n=%d): mean %+.2f dB   SD of note loudness: %.2f dB" % (
        pct(C["ghost"], len(S["rdb"])), len(S["longdb"]), st.mean(S["longdb"]) if S["longdb"] else float("nan"), st.pstdev(S["rdb"])))
    P("C. MOTIFS")
    P("  midlevel units marked '#' (variant of an earlier idea): %s of %d" % (pct(C["ideas_var"], C["ideas"]), C["ideas"]))
    tc = Counter(S["restate_t"])
    P("  phrases with a 4-note cell restated immediately: %s (%d of %d); transpositions: %s" % (
        pct(C["restate"], C["ph_counted"]), C["restate"], C["ph_counted"],
        ", ".join("%+d x%d" % kv for kv in sorted(tc.items(), key=lambda kv: (-kv[1], kv[0])))))
    P("")


# ---------------------------------------------------------------- licks (as in the lick report)
DEG = {0: "1", 1: "b9", 2: "9", 4: "3", 7: "5", 10: "b7", 11: "7"}


def degree(r, ch):
    kind = ch[1] if ch else None
    if r == 5:
        return "4" if kind == "sus" else "11"
    if r in DEG:
        return DEG[r]
    if r == 3:
        return "b3" if kind in ("min", "mintonic", "hdim", "dim") else "#9"
    if r == 6:
        return "b5" if kind in ("hdim", "dim") else "#11"
    if r == 8:
        return "#5" if kind == "aug" else "b13"
    if r == 9:
        return "bb7" if kind == "dim" else ("6" if kind in ("maj6", "mintonic", "maj") else "13")
    return "?"


def key_pc_of(k):
    m = re.match(r"([A-G])([b#]?)", k or "")
    return None if not m else (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12


def function(ch, nxt, key_pc):
    if ch is None:
        return "no chord"
    root, kind = ch
    iv = (nxt[0] - root) % 12 if nxt else None
    if kind == "min":
        return "ii-V (ii)" if nxt and nxt[1] in ("dom", "sus", "aug") and iv == 5 else "m7 static/modal"
    if kind == "hdim":
        return "minor ii-V (iiø)" if nxt and iv == 5 else "m7b5"
    if kind in ("dom", "aug", "sus"):
        if nxt and iv == 5:
            return "V7->I (V)" if nxt[1] not in ("dom", "aug", "sus") else "V7 cycle (dominant chain)"
        if nxt and iv == 11 and kind == "dom" and nxt[1] not in ("dom", "sus", "aug"):
            return "tritone sub V (bII7->I)"
        if key_pc is not None and root == key_pc:
            return "I7 (blues tonic)"
        if key_pc is not None and root == (key_pc + 5) % 12:
            return "IV7 (blues)"
        return "V7 static/other dom"
    if kind == "dim":
        return "dim7 (passing)"
    if kind in ("maj", "maj6"):
        return "I (tonic major)" if key_pc is not None and root == key_pc else "maj (non-tonic)"
    return "i (minor tonic)" if kind == "mintonic" else kind


def metpos(n):
    lab = {Fraction(0): "", Fraction(1, 4): "e", Fraction(1, 2): "&", Fraction(3, 4): "a",
           Fraction(1, 3): "(trip2)", Fraction(2, 3): "(trip3)"}.get(n["frac"], "+%s" % n["frac"])
    return "%d%s" % (n["beat"], lab)


def nname(p):
    return "%s%d" % (NAMES[p % 12], p // 12 - 1)


def fmt_beats(x):
    x = Fraction(round(x * 12), 12)
    if x.denominator == 1:
        return str(x.numerator)
    w, r = divmod(x.numerator, x.denominator)
    return ("%d+%d/%d" % (w, r, x.denominator)) if w else "%d/%d" % (r, x.denominator)


DURS = [(Fraction(1, 4), "16"), (Fraction(1, 3), "12"), (Fraction(1, 2), "8"), (Fraction(2, 3), "6"),
        (Fraction(3, 4), "8."), (Fraction(1), "4"), (Fraction(3, 2), "4."), (Fraction(2), "2"), (Fraction(3), "2."), (Fraction(4), "1")]


def mml(pitches, lens):
    s, cur = [], None
    for p, l in zip(pitches, lens):
        o = p // 12 - 1
        if o != cur:
            s.append("o%d" % o); cur = o
        s.append(MML[p % 12] + min(DURS, key=lambda d: abs(float(d[0]) - l))[1])
    return " ".join(s)


def contour(iv):
    return "".join("+" if x > 0 else "-" if x < 0 else "0" for x in iv)


def is_scale_run(iv):
    return all(1 <= x <= 2 for x in iv) or all(-2 <= x <= -1 for x in iv)


def npitches(iv):
    return len(set(itertools.accumulate((0,) + iv)))


def licks(solos, label):
    occ = defaultdict(list)
    data = {s["melid"]: s["data"] for s in solos}
    info = {s["melid"]: s for s in solos}
    for s in solos:
        d = s["data"]
        for a, b in d["phrases"]:
            ps = [x["pitch"] for x in d["notes"][a:b + 1]]
            for L in range(5, 11):
                last_end = {}
                for i in range(len(ps) - L + 1):
                    iv = tuple(ps[i + k + 1] - ps[i + k] for k in range(L - 1))
                    if last_end.get(iv, -1) > i:
                        continue
                    last_end[iv] = i + L - 1
                    occ[iv].append((s["melid"], a + i))
    cnt = {iv: len(v) for iv, v in occ.items()}
    nsol = {iv: len({m for m, _ in v}) for iv, v in occ.items()}
    cands = [iv for iv in occ if nsol[iv] >= 2 and cnt[iv] >= 2 and any(iv)]
    ext = defaultdict(int)
    for iv in occ:
        if len(iv) >= 5:
            ext[iv[1:]] = max(ext[iv[1:]], cnt[iv])
            ext[iv[:-1]] = max(ext[iv[:-1]], cnt[iv])
    cands = [iv for iv in cands if ext.get(iv, 0) < 0.8 * cnt[iv]]
    cands.sort(key=lambda iv: (-(cnt[iv] * (len(iv) + 1)), -nsol[iv], -len(iv)))

    def related(a, b):
        sa, sb = "," + ",".join(map(str, a)) + ",", "," + ",".join(map(str, b)) + ","
        if sa in sb or sb in sa:
            return True
        m = min(len(a), len(b)) - 1
        return m >= 3 and (a[:m] == b[-m:] or b[:m] == a[-m:])

    kept, runs = [], 0
    for iv in cands:
        if any(related(iv, k) for k in kept) or npitches(iv) <= 2:
            continue
        if is_scale_run(iv):
            if runs >= 2:
                continue
            runs += 1
        kept.append(iv)
        if len(kept) == 12:
            break
    longs = []
    for iv in sorted((iv for iv in cands if len(iv) + 1 >= 7 and iv not in kept), key=lambda iv: (-nsol[iv], -cnt[iv], -len(iv))):
        if npitches(iv) <= 3 or is_scale_run(iv) or any(related(iv, k) for k in kept + longs):
            continue
        longs.append(iv)
        if len(longs) == 2:
            break

    if not longs:
        P("  (no separate 'longer' licks: the only recurring 7+ note pattern in >= 2 solos that is")
        P("   not a trill/run or a variant of a main lick is already main lick #5 below)" if any(len(k) >= 6 for k in kept)
          else "  (no recurring 7+ note pattern in >= 2 solos qualified)")
    P("D. LICKS (%s; method as in the lick report: 5-10 note interval patterns within phrases," % label)
    P("   in >= 2 solos, closed, ranked occurrences x length; examples exactly as played)")
    S_len, S_pos, S_up, S_land, S_func, S_cont, S_coarse = Counter(), Counter(), [0, 0], [0, 0], Counter(), Counter(), Counter()
    for rank, iv, main in [("#%d" % r, iv, True) for r, iv in enumerate(kept, 1)] + \
            [("longer #%d" % r, iv, False) for r, iv in enumerate(longs, 1)]:
        if not main and rank.endswith("#1"):
            P("")
            P("  (longer licks: the 2 recurring patterns of 7+ notes found in the most solos)")
        L = len(iv) + 1
        details = []
        for melid, s0 in occ[iv]:
            d = data[melid]
            n, chords, csym, cidx = d["notes"], d["chords"], d["csym"], d["cidx"]
            ch = chords[s0]
            fd = degree((n[s0]["pitch"] - ch[0]) % 12, ch) if ch else "?"
            nxt = d["nextch"].get(cidx[s0], (None, None)) if cidx[s0] is not None else (None, None)
            for k in range(s0 + 1, s0 + L):
                if cidx[k] is not None and cidx[k] != cidx[s0] and csym[k] != csym[s0]:
                    nxt = (chords[k], csym[k]); break
            fn = function(ch, nxt[0], key_pc_of(info[melid]["key"]))
            details.append((melid, s0, fd, fn))
            if not main:
                continue
            n0 = n[s0]
            S_pos[metpos(n0)] += 1
            S_up[0] += n0["frac"] != 0; S_up[1] += 1
            S_func[fn] += 1; S_len[L] += 1
            S_cont[contour(iv)] += 1; S_coarse[re.sub(r"(.)\1+", r"\1", contour(iv))] += 1
            last = n[s0 + L - 1]
            num = last["num"]
            strong = [0, 2] if num == 4 else [0]
            barstart = last["pos"] - (last["beat"] - 1) - last["frac"]
            tb = next(b for b in sorted(barstart + k * num + sb for k in range(3) for sb in strong) if b >= last["pos"])
            j = s0 + L - 1
            while j + 1 < len(n) and n[j + 1]["pos"] <= tb and n[j + 1]["pos"] - n[j]["pos"] < 4:
                j += 1
            if chords[j]:
                S_land[1] += 1
                S_land[0] += is_ct(n[j]["pitch"], chords[j])
        degc = Counter(x[2] for x in details)
        fc = Counter(x[3] for x in details)
        melid, s0, _, fn = next(x for x in details if x[2] == degc.most_common(1)[0][0])
        d = data[melid]
        n, chords, csym, cidx = d["notes"], d["chords"], d["csym"], d["cidx"]
        seg = n[s0:s0 + L]
        ch = chords[s0]
        pitches = [x["pitch"] for x in seg]
        ioi = [float(seg[k + 1]["pos"] - seg[k]["pos"]) for k in range(L - 1)]
        lastlen = seg[-1]["dur"] / seg[-1]["bd"]
        if s0 + L < len(n):
            lastlen = min(lastlen, float(n[s0 + L]["pos"] - seg[-1]["pos"])) or lastlen
        ioi.append(lastlen)
        syms = []
        for k in range(s0, s0 + L):
            if csym[k] and (not syms or syms[-1] != csym[k]):
                syms.append(csym[k])
        nx = d["nextch"].get(cidx[s0 + L - 1]) if cidx[s0 + L - 1] is not None else None
        s = info[melid]
        P("")
        P("%s  intervals [%s]  (%d notes)  occurrences %d in %d solos  contour %s" % (
            rank, " ".join("%+d" % x for x in iv), L, cnt[iv], nsol[iv], contour(iv)))
        P("  notes (as played): " + " ".join(nname(p) for p in pitches))
        P("  degrees:           " + " ".join(degree((p - ch[0]) % 12, ch) if ch else "?" for p in pitches))
        P("  grid starts:       " + " ".join(fmt_beats(x["pos"] - seg[0]["pos"]) for x in seg))
        P("  metric pos:        " + " ".join(metpos(x) for x in seg) + "   (bar %d)" % seg[0]["bar"])
        P("  played onsets (s): " + " ".join("%.3f" % (x["onset"] - seg[0]["onset"]) for x in seg))
        P("  played durs (s):   " + " ".join("%.3f" % x["dur"] for x in seg) + "   (beat = %.3f s)" % seg[0]["bd"])
        P("  chords: %s%s   function: %s" % (" | ".join(syms) or "none",
                                            ("  (next: %s)" % nx[1]) if nx and nx[1] not in syms else "", fn))
        P("  from: %s (%s), %d bpm, key %s, at %.2f s into the solo" % (s["title"], s["year"], round(s["tempo"] or 0), s["key"] or "?", seg[0]["onset"]))
        P("  MML (as played): " + mml(pitches, ioi))
        if main:
            P("  first-note degree across occurrences: " + ", ".join("%s x%d" % kv for kv in degc.most_common(4)) +
              "; functions: " + ", ".join("%s x%d" % kv for kv in fc.most_common(3)))
    tot = sum(S_len.values()) or 1
    P("")
    P("-- %s lick summary (over all %d occurrences of the %d main licks) --" % (label, tot, len(kept)))
    P("  lick lengths (notes): " + ", ".join("%d: %d%%" % (k, round(100 * v / tot)) for k, v in S_len.most_common()))
    P("  commonest start positions: " + ", ".join("%s %d%%" % (k, round(100 * v / tot)) for k, v in S_pos.most_common(6)))
    P("  starting off the beat (upbeat/any subdivision): %d%%" % round(100 * S_up[0] / max(1, S_up[1])))
    P("  landing on a chord tone at the next strong beat: %d%% (%d/%d with a chord)" % (
        round(100 * S_land[0] / max(1, S_land[1])), S_land[0], S_land[1]))
    P("  chord functions: " + ", ".join("%s %d%%" % (k, round(100 * v / tot)) for k, v in S_func.most_common(5)))
    P("  top contours (exact): " + ", ".join("%s x%d" % kv for kv in S_cont.most_common(3)))
    P("  top contours (runs collapsed): " + ", ".join("%s x%d" % kv for kv in S_coarse.most_common(3)))


arm = solos_where(lambda p, y: p == "Louis Armstrong")
pre = solos_where(lambda p, y: y is not None and y < 1931)
P("LOUIS ARMSTRONG -- Weimar Jazz Database (wjazzd.db, jazzomat.hfm-weimar.de)")
P("=" * 74)
P("Solos: " + "; ".join("%s (%s, %s, %.0f bpm)" % (s["title"], s["year"], s["instr"], s["tempo"]) for s in arm))
P("Comparison row: all pre-1931 solos in the DB (%d: %s)." % (len(pre), ", ".join(
    "%s x%d" % kv for kv in Counter(s["perf"] for s in pre).most_common())))
P("Notes: pitches are concert; phrases = the DB's PHRASE sections; 'strong beats' = 1 and 3")
P("(1 in 3/4); IOI classes use the transcription's metrical grid; loudness = loud_med")
P("(dB) relative to each solo's median; swing ratio = (8th 'and' onset - beat onset) /")
P("(next beat onset - 'and' onset) for a beat split 1+&; timing = note onset minus the")
P("annotated beat onset for notes on a beat.")
P("")
stats(arm, "Louis Armstrong")
stats(pre, "All pre-1931 solos")
licks(arm, "Louis Armstrong")
print("\n".join(out_lines))
