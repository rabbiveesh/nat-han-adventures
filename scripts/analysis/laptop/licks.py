import sqlite3, re, sys
from collections import Counter, defaultdict
from fractions import Fraction

DB = sys.argv[1]
c = sqlite3.connect(DB)

PLAYERS = [
    ("Miles Davis 1951-59", "Miles Davis", lambda y: 1951 <= y <= 1959),
    ("Miles Davis 1964-69", "Miles Davis", lambda y: 1964 <= y <= 1969),
    ("John Coltrane <1960", "John Coltrane", lambda y: y < 1960),
    ("John Coltrane 1960+", "John Coltrane", lambda y: y >= 1960),
    ("Charlie Parker", "Charlie Parker", lambda y: True),
    ("Sonny Rollins", "Sonny Rollins", lambda y: True),
    ("Wayne Shorter", "Wayne Shorter", lambda y: True),
]
MINLEN, MAXLEN, KEEP = 5, 10, 12

PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
NAMES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
MML = ["c", "d-", "d", "e-", "e", "f", "g-", "g", "a-", "a", "b-", "b"]


def parse_chord(sym):
    if not sym or sym == "NC":
        return None
    sym = sym.split("/")[0]
    m = re.match(r"([A-G])([b#]?)(.*)", sym)
    if not m:
        return None
    root = (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12
    q = m.group(3)
    if q.startswith("m7b5"):
        kind = "hdim"
    elif q.startswith("o"):
        kind = "dim"
    elif q.startswith("-"):
        kind = "min"
    elif q.startswith("+"):
        kind = "aug"
    elif q.startswith("sus"):
        kind = "sus"
    elif q.startswith("j7"):
        kind = "maj"
    elif q.startswith("7"):
        kind = "dom"
    elif q.startswith("6"):
        kind = "maj6"
    else:
        kind = "maj"  # triad
    if kind == "min" and ("6" in q[:2] or "j7" in q):
        kind = "mintonic"
    seventh = "7" in q or kind in ("hdim",)
    return root, kind, seventh, q


DEG = {0: "1", 1: "b9", 2: "9", 4: "3", 5: "11", 7: "5", 10: "b7", 11: "7"}


def degree(pc_rel, ch):
    kind = ch[1] if ch else None
    if pc_rel == 5:
        return "4" if kind == "sus" else "11"
    if pc_rel in DEG:
        return DEG[pc_rel]
    if pc_rel == 3:
        return "b3" if kind in ("min", "mintonic", "hdim", "dim") else "#9"
    if pc_rel == 6:
        return "b5" if kind in ("hdim", "dim") else "#11"
    if pc_rel == 8:
        return "#5" if kind == "aug" else "b13"
    if pc_rel == 9:
        return "bb7" if kind == "dim" else ("6" if kind in ("maj6", "mintonic", "maj") else "13")
    return "?"


def chord_tones(ch):
    kind = ch[1]
    third = 3 if kind in ("min", "mintonic", "hdim", "dim") else (5 if kind == "sus" else 4)
    fifth = 6 if kind in ("hdim", "dim") else (8 if kind == "aug" else 7)
    sev = {"maj": {11, 9}, "maj6": {9, 11}, "mintonic": {9, 11}, "dim": {9}, "hdim": {10}}.get(kind, {10})
    return {0, third, fifth} | sev


def function(ch, nxt, key_pc):
    if ch is None:
        return "no chord"
    root, kind = ch[0], ch[1]
    iv = (nxt[0] - root) % 12 if nxt else None
    if kind == "min":
        if nxt and nxt[1] in ("dom", "sus", "aug") and iv == 5:
            return "ii-V (ii)"
        return "m7 static/modal"
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
        if key_pc is not None and root == key_pc:
            return "I (tonic major)"
        return "maj7 (non-tonic)"
    if kind == "mintonic":
        return "i (minor tonic)"
    return kind


def year_of(s):
    m = re.findall(r"(19\d\d)", s or "")
    return int(m[-1]) if m else None


def key_pc_of(k):
    m = re.match(r"([A-G])([b#]?)", k or "")
    if not m:
        return None
    return (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12


def load_solo(melid):
    notes = list(c.execute(
        "select onset,pitch,duration,bar,beat,tatum,division,num,beatdur from melody where melid=? order by eventid",
        (melid,)))
    # beats per bar from the beat track
    bpb = defaultdict(int)
    for bar, beat in c.execute("select bar,beat from beats where melid=?", (melid,)):
        bpb[bar] = max(bpb[bar], beat)
    bars = sorted(bpb)
    start, acc = {}, 0
    for b in bars:
        start[b] = acc
        acc += bpb[b]
    out = []
    for (onset, pitch, dur, bar, beat, tatum, div, num, bd) in notes:
        bs = start.get(bar, bar * (num or 4))
        frac = Fraction(tatum - 1, div) if div else Fraction(0)
        out.append(dict(onset=onset, pitch=int(round(pitch)), dur=dur, bar=bar, beat=beat,
                        frac=frac, pos=bs + (beat - 1) + frac, bd=bd, num=num))
    chords = [None] * len(out)
    csym = [None] * len(out)
    cidx = [None] * len(out)
    secs = list(c.execute("select start,end,value from sections where melid=? and type='CHORD' order by start", (melid,)))
    for i, (s, e, v) in enumerate(secs):
        for k in range(s, min(e, len(out) - 1) + 1):
            chords[k] = parse_chord(v)
            csym[k] = v
            cidx[k] = i
    nextch = {}
    for i in range(len(secs)):
        for j in range(i + 1, len(secs)):
            if secs[j][2] != secs[i][2]:
                nextch[i] = (parse_chord(secs[j][2]), secs[j][2])
                break
    phrases = [(s, e) for s, e in c.execute("select start,end from sections where melid=? and type='PHRASE' order by start", (melid,))]
    if not phrases:
        phrases, s = [], 0
        for i in range(1, len(out)):
            if (out[i]["onset"] - out[i - 1]["onset"] - out[i - 1]["dur"]) / out[i - 1]["bd"] >= 0.5:
                phrases.append((s, i - 1)); s = i
        phrases.append((s, len(out) - 1))
    return out, chords, csym, cidx, nextch, phrases


def metpos(n):
    f = n["frac"]
    lab = {Fraction(0): "", Fraction(1, 4): "e", Fraction(1, 2): "&", Fraction(3, 4): "a",
           Fraction(1, 3): "(trip2)", Fraction(2, 3): "(trip3)"}.get(f, "+%s" % f)
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
        d = min(DURS, key=lambda d: abs(float(d[0]) - l))[1]
        s.append(MML[p % 12] + d)
    return " ".join(s)


def contour(iv):
    return "".join("+" if x > 0 else "-" if x < 0 else "0" for x in iv)


def coarse(iv):
    s = contour(iv)
    return re.sub(r"(.)\1+", r"\1", s)


def is_scale_run(iv):
    return all(1 <= x <= 2 for x in iv) or all(-2 <= x <= -1 for x in iv)


report = []
P = report.append

P("LICK ANALYSIS -- Weimar Jazz Database (wjazzd.db, jazzomat.hfm-weimar.de)")
P("=" * 74)
P("Method: per player, every note window of %d-%d notes inside one phrase (the DB's" % (MINLEN, MAXLEN))
P("PHRASE sections) is keyed by its semitone interval sequence (rhythm-free, any")
P("transposition). Occurrences are counted non-overlapping; 'solos' = distinct solos")
P("containing it. Only patterns in >= 2 solos qualify. Closed patterns only (a pattern")
P("is dropped if a one-note extension of it occurs at least 80% as often). Ranked by")
P("occurrences x length; top %d kept, skipping patterns that are a sub/super-sequence" % KEEP)
P("of, or overlap all but one interval with, a kept lick; trivial patterns (only one or")
P("two distinct pitches: repeats, trills) skipped;")
P("one-direction stepwise runs capped at 2.")
P("Representative occurrence = the one whose first note has the commonest scale degree")
P("over its chord among all occurrences. Notes are transposed so the CHORD ROOT (at the")
P("first note) = C, kept near the original register. Degrees: relative to the chord at")
P("the first note (b3 on minor/dim chords, #9 on major/dominant; 6 on maj/maj6, 13 on")
P("dominant). Rhythm: start in beats from the first note (1/12 resolution) and the")
P("metrical position (beat + e/&/a sixteenths, (trip2)/(trip3) triplet 8ths). MML uses")
P("each note's inter-onset time snapped to 16 / 12 (triplet 8th) / 8 / 6 / 8. / 4 ...")
P("Landing = the note sounding at the first strong beat (1 or 3; 1 in 3/4) at/after")
P("the lick's last note, checked against the chord there (1,3,5,7/6).")
P("Degrees are relative to the first chord on each lick's 'chords:' line. Summaries")
P("cover the 12 main licks; each player also gets 2 'longer' licks (7+ notes, ranked")
P("by number of solos) because occurrences x length favours 5-note cells.")
P("")

for label, performer, yf in PLAYERS:
    solos = []
    for melid, title, rdate, key, tempo, sig in c.execute(
            "select s.melid,s.title,t.recordingdate,s.key,s.avgtempo,s.signature from solo_info s join track_info t using(trackid) where performer=? order by melid",
            (performer,)):
        y = year_of(rdate)
        if y is None or not yf(y):
            continue
        solos.append((melid, title, y, key, tempo, sig))
    data = {}
    occ = defaultdict(list)  # pattern -> list of (melid, start)
    nnotes = 0
    for melid, title, y, key, tempo, sig in solos:
        out, chords, csym, cidx, nextch, phrases = load_solo(melid)
        data[melid] = (out, chords, csym, cidx, nextch, phrases)
        nnotes += len(out)
        for s, e in phrases:
            ps = [n["pitch"] for n in out[s:e + 1]]
            for L in range(MINLEN, MAXLEN + 1):
                last_end = {}
                for i in range(0, len(ps) - L + 1):
                    iv = tuple(ps[i + k + 1] - ps[i + k] for k in range(L - 1))
                    if last_end.get(iv, -1) > i:
                        continue  # overlapping self-occurrence
                    last_end[iv] = i + L - 1
                    occ[iv].append((melid, s + i))
    cnt = {iv: len(v) for iv, v in occ.items()}
    nsol = {iv: len({m for m, _ in v}) for iv, v in occ.items()}
    cands = [iv for iv in occ if nsol[iv] >= 2 and cnt[iv] >= 2 and any(x != 0 for x in iv)]
    # closedness: drop if an extension one longer keeps >= 80% of the count
    ext = defaultdict(int)
    for iv in occ:
        if len(iv) >= MINLEN:
            ext[iv[1:]] = max(ext[iv[1:]], cnt[iv])
            ext[iv[:-1]] = max(ext[iv[:-1]], cnt[iv])
    cands = [iv for iv in cands if ext.get(iv, 0) < 0.8 * cnt[iv]]
    cands.sort(key=lambda iv: (-(cnt[iv] * (len(iv) + 1)), -nsol[iv], -len(iv)))

    def related(a, b):
        sa, sb = ",".join(map(str, a)) + ",", ",".join(map(str, b)) + ","
        if len(a) <= len(b) and ("," + sa) in ("," + sb):
            return True
        if len(b) < len(a) and ("," + sb) in ("," + sa):
            return True
        m = min(len(a), len(b)) - 1
        return m >= 3 and (a[:m] == b[-m:] or b[:m] == a[-m:])

    kept, runs = [], 0
    for iv in cands:
        if any(related(iv, k) for k in kept):
            continue
        if len(set(__import__("itertools").accumulate((0,) + iv))) <= 2:
            continue
        if is_scale_run(iv):
            if runs >= 2:
                continue
            runs += 1
        kept.append(iv)
        if len(kept) == KEEP:
            break

    longs = []
    for iv in sorted((iv for iv in cands if len(iv) + 1 >= 7 and iv not in kept),
                     key=lambda iv: (-nsol[iv], -cnt[iv], -len(iv))):
        if len(set(__import__("itertools").accumulate((0,) + iv))) <= 3 or is_scale_run(iv):
            continue
        if any(related(iv, k) for k in kept + longs):
            continue
        longs.append(iv)
        if len(longs) == 2:
            break

    info = {m: (t, y, k, tp, sg) for m, t, y, k, tp, sg in solos}
    P("#" * 74)
    P("%s   (%d solos, %d notes; years %s)" % (label.upper(), len(solos), nnotes,
                                               ", ".join(sorted({str(s[2]) for s in solos}))))
    P("solos: " + "; ".join("%s (%d, %d bpm)" % (t, y, round(tp or 0)) for m, t, y, k, tp, sg in solos))
    P("#" * 74)

    S_len, S_pos, S_up, S_land, S_func, S_cont, S_coarse = Counter(), Counter(), [0, 0], [0, 0], Counter(), Counter(), Counter()
    for rank, iv, main in [("#%d" % r, iv, True) for r, iv in enumerate(kept, 1)] + \
            [("longer #%d" % r, iv, False) for r, iv in enumerate(longs, 1)]:
        if not main and rank.endswith("#1"):
            P("")
            P("  (longer licks: the 2 recurring patterns of 7+ notes found in the most solos)")
        L = len(iv) + 1
        details = []
        for melid, st in occ[iv]:
            out, chords, csym, cidx, nextch, phrases = data[melid]
            ch = chords[st]
            first_deg = degree((out[st]["pitch"] - ch[0]) % 12, ch) if ch else "?"
            nxt = nextch.get(cidx[st], (None, None)) if cidx[st] is not None else (None, None)
            # chord change inside the lick counts as the "next" chord
            for k in range(st + 1, st + L):
                if cidx[k] is not None and cidx[k] != cidx[st] and csym[k] != csym[st]:
                    nxt = (chords[k], csym[k]); break
            fn = function(ch, nxt[0], key_pc_of(info[melid][2]))
            details.append((melid, st, first_deg, fn))
            if not main:
                continue
            # summary stats
            n0 = out[st]
            S_pos[metpos(n0)] += 1
            S_up[0] += n0["frac"] != 0; S_up[1] += 1
            S_func[fn] += 1
            S_len[L] += 1
            S_cont[contour(iv)] += 1
            S_coarse[coarse(iv)] += 1
            lastn = out[st + L - 1]
            num = n0["num"] or 4
            strong = [0, 2] if num == 4 else [0]
            # first strong beat at/after last note onset
            barstart = lastn["pos"] - (lastn["beat"] - 1) - lastn["frac"]
            cand_beats = sorted(barstart + k * num + sb for k in range(0, 3) for sb in strong)
            tb = next(b for b in cand_beats if b >= lastn["pos"])
            j = st + L - 1
            while j + 1 < len(out) and out[j + 1]["pos"] <= tb and out[j + 1]["pos"] - out[j]["pos"] < 4:
                j += 1
            if chords[j]:
                S_land[1] += 1
                S_land[0] += ((out[j]["pitch"] - chords[j][0]) % 12) in chord_tones(chords[j])
        degc = Counter(d for _, _, d, _ in details)
        fc = Counter(f for _, _, _, f in details)
        top_deg = degc.most_common(1)[0][0]
        melid, st, _, fn = next(d for d in details if d[2] == top_deg)
        out, chords, csym, cidx, nextch, phrases = data[melid]
        seg = out[st:st + L]
        ch = chords[st]
        shift = 0
        if ch:
            shift = -ch[0] if ch[0] <= 6 else 12 - ch[0]
        tp = [n["pitch"] + shift for n in seg]
        degs = [degree((n["pitch"] - ch[0]) % 12, ch) if ch else "?" for n in seg]
        rel = [n["pos"] - seg[0]["pos"] for n in seg]
        ioi = [float(seg[k + 1]["pos"] - seg[k]["pos"]) for k in range(L - 1)]
        lastlen = seg[-1]["dur"] / seg[-1]["bd"] if seg[-1]["bd"] else 0.5
        if st + L < len(out):
            lastlen = min(lastlen, float(out[st + L]["pos"] - seg[-1]["pos"])) or lastlen
        ioi.append(lastlen)
        syms = []
        for k in range(st, st + L):
            if csym[k] and (not syms or syms[-1] != csym[k]):
                syms.append(csym[k])
        nx = nextch.get(cidx[st + L - 1]) if cidx[st + L - 1] is not None else None
        t, y, key, tempo, sig = info[melid]
        P("")
        P("%s  intervals [%s]  (%d notes)  occurrences %d in %d solos  contour %s" % (
            rank, " ".join("%+d" % x for x in iv), L, cnt[iv], nsol[iv], contour(iv)))
        P("  notes (root=C): " + " ".join(nname(p) for p in tp))
        P("  degrees:        " + " ".join(degs))
        P("  starts (beats): " + " ".join(fmt_beats(r) for r in rel))
        P("  metric pos:     " + " ".join(metpos(n) for n in seg) + "   (starts bar %d)" % seg[0]["bar"])
        P("  chords: %s%s   function: %s" % (" | ".join(syms) or "none",
                                            ("  (next: %s)" % nx[1]) if nx and nx[1] not in syms else "", fn))
        P("  from: %s (%d), %d bpm, key %s, 1st note at %.1f s" % (t, y, round(tempo or 0), key or "?", seg[0]["onset"]))
        P("  MML (root=C): " + mml(tp, ioi))
        if main:
            P("  first-note degree across occurrences: " + ", ".join("%s x%d" % kv for kv in degc.most_common(4)) +
          "; functions: " + ", ".join("%s x%d" % kv for kv in fc.most_common(3)))

    tot = sum(S_len.values()) or 1
    P("")
    P("-- %s summary (over all %d occurrences of the %d kept licks) --" % (label, tot, len(kept)))
    P("  lick lengths (notes): " + ", ".join("%d: %d%%" % (k, round(100 * v / tot)) for k, v in sorted(S_len.items(), key=lambda kv: -kv[1])))
    P("  commonest start positions: " + ", ".join("%s %d%%" % (k, round(100 * v / tot)) for k, v in S_pos.most_common(6)))
    P("  starting off the beat (upbeat/any subdivision): %d%%" % round(100 * S_up[0] / max(1, S_up[1])))
    P("  landing on a chord tone at the next strong beat: %d%% (%d/%d with a chord)" % (
        round(100 * S_land[0] / max(1, S_land[1])), S_land[0], S_land[1]))
    P("  chord functions: " + ", ".join("%s %d%%" % (k, round(100 * v / tot)) for k, v in S_func.most_common(5)))
    P("  top contours (exact): " + ", ".join("%s x%d" % kv for kv in S_cont.most_common(3)))
    P("  top contours (runs collapsed): " + ", ".join("%s x%d" % kv for kv in S_coarse.most_common(3)))
    P("")

print("\n".join(report))
