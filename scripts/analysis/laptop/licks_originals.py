import sqlite3, re, sys, itertools
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
        kind = "maj"
    if kind == "min" and ("6" in q[:2] or "j7" in q):
        kind = "mintonic"
    return root, kind


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


def chord_tones(ch):
    kind = ch[1]
    third = 3 if kind in ("min", "mintonic", "hdim", "dim") else (5 if kind == "sus" else 4)
    fifth = 6 if kind in ("hdim", "dim") else (8 if kind == "aug" else 7)
    sev = {"maj": {11, 9}, "maj6": {9, 11}, "mintonic": {9, 11}, "dim": {9}, "hdim": {10}}.get(kind, {10})
    return {0, third, fifth} | sev


def function(ch, nxt, key_pc):
    if ch is None:
        return "no chord"
    root, kind = ch
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
        return "I (tonic major)" if key_pc is not None and root == key_pc else "maj7 (non-tonic)"
    if kind == "mintonic":
        return "i (minor tonic)"
    return kind


def year_of(s):
    m = re.findall(r"(19\d\d)", s or "")
    return int(m[-1]) if m else None


def key_pc_of(k):
    m = re.match(r"([A-G])([b#]?)", k or "")
    return None if not m else (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12


def load_solo(melid):
    notes = list(c.execute(
        "select onset,pitch,duration,bar,beat,tatum,division,num,beatdur from melody where melid=? order by eventid",
        (melid,)))
    bpb = defaultdict(int)
    for bar, beat in c.execute("select bar,beat from beats where melid=?", (melid,)):
        bpb[bar] = max(bpb[bar], beat)
    start, acc = {}, 0
    for b in sorted(bpb):
        start[b] = acc
        acc += bpb[b]
    out = []
    for (onset, pitch, dur, bar, beat, tatum, div, num, bd) in notes:
        bs = start.get(bar, bar * (num or 4))
        frac = Fraction(tatum - 1, div) if div else Fraction(0)
        out.append(dict(onset=onset, pitch=int(round(pitch)), dur=dur, bar=bar, beat=beat,
                        frac=frac, pos=bs + (beat - 1) + frac, bd=bd, num=num))
    chords, csym, cidx = [None] * len(out), [None] * len(out), [None] * len(out)
    secs = list(c.execute("select start,end,value from sections where melid=? and type='CHORD' order by start", (melid,)))
    for i, (s, e, v) in enumerate(secs):
        for k in range(s, min(e, len(out) - 1) + 1):
            chords[k], csym[k], cidx[k] = parse_chord(v), v, i
    nextch = {}
    for i in range(len(secs)):
        for j in range(i + 1, len(secs)):
            if secs[j][2] != secs[i][2]:
                nextch[i] = (parse_chord(secs[j][2]), secs[j][2])
                break
    phrases = list(c.execute("select start,end from sections where melid=? and type='PHRASE' order by start", (melid,)))
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
        s.append(MML[p % 12] + min(DURS, key=lambda d: abs(float(d[0]) - l))[1])
    return " ".join(s)


def contour(iv):
    return "".join("+" if x > 0 else "-" if x < 0 else "0" for x in iv)


def coarse(iv):
    return re.sub(r"(.)\1+", r"\1", contour(iv))


def is_scale_run(iv):
    return all(1 <= x <= 2 for x in iv) or all(-2 <= x <= -1 for x in iv)


def npitches(iv):
    return len(set(itertools.accumulate((0,) + iv)))


report = []
P = report.append
P("LICK ANALYSIS -- Weimar Jazz Database (wjazzd.db, jazzomat.hfm-weimar.de) -- ORIGINALS")
P("=" * 74)
P("Same licks as before; every example is now the EXACT original: notes as played")
P("in the database's concert pitch and original key, with real timing.")
P("Method: per player, every note window of 5-10 notes inside one phrase (the DB's")
P("PHRASE sections) is keyed by its semitone interval sequence (rhythm-free, any")
P("transposition). Occurrences counted non-overlapping; 'solos' = distinct solos")
P("containing it. Only patterns in >= 2 solos qualify; closed patterns only (dropped if")
P("a one-note extension occurs >= 80% as often). Ranked by occurrences x length; top 12,")
P("skipping sub/super-sequences of (or overlaps by all but one interval with) a kept")
P("lick; trills/repeats (<= 2 distinct pitches) skipped; one-direction stepwise runs")
P("capped at 2. Each player also gets 2 'longer' licks (7+ notes, ranked by solos).")
P("Example shown = the occurrence whose first note has the commonest degree over its")
P("chord among all occurrences.")
P("Per lick: notes as played (concert pitch, original octave); degrees relative to")
P("the first chord on the 'chords:' line; grid starts in beats from the first note")
P("(the transcription's metric grid, 1/12 beat) and metrical position (e/&/a")
P("sixteenths, (trip2)/(trip3) triplet 8ths, +k/n other tuplets); 'played' = real")
P("onsets in seconds after the first note and each note's real duration in seconds;")
P("MML in the original key, inter-onset times snapped to 16/12/8/6/8./4...")
P("Landing = the note sounding at the first strong beat (1 or 3; 1 in 3/4) at/after")
P("the lick's last note, checked against the chord there (1,3,5,7/6).")
P("")

for label, performer, yf in PLAYERS:
    solos = []
    for melid, title, rdate, key, tempo in c.execute(
            "select s.melid,s.title,t.recordingdate,s.key,s.avgtempo from solo_info s join track_info t using(trackid) where performer=? order by melid",
            (performer,)):
        y = year_of(rdate)
        if y is not None and yf(y):
            solos.append((melid, title, y, key, tempo))
    data, occ, nnotes = {}, defaultdict(list), 0
    for melid, *_ in solos:
        out, chords, csym, cidx, nextch, phrases = data[melid] = load_solo(melid)
        nnotes += len(out)
        for s, e in phrases:
            ps = [n["pitch"] for n in out[s:e + 1]]
            for L in range(MINLEN, MAXLEN + 1):
                last_end = {}
                for i in range(0, len(ps) - L + 1):
                    iv = tuple(ps[i + k + 1] - ps[i + k] for k in range(L - 1))
                    if last_end.get(iv, -1) > i:
                        continue
                    last_end[iv] = i + L - 1
                    occ[iv].append((melid, s + i))
    cnt = {iv: len(v) for iv, v in occ.items()}
    nsol = {iv: len({m for m, _ in v}) for iv, v in occ.items()}
    cands = [iv for iv in occ if nsol[iv] >= 2 and cnt[iv] >= 2 and any(x != 0 for x in iv)]
    ext = defaultdict(int)
    for iv in occ:
        if len(iv) >= MINLEN:
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
        if len(kept) == KEEP:
            break
    longs = []
    for iv in sorted((iv for iv in cands if len(iv) + 1 >= 7 and iv not in kept),
                     key=lambda iv: (-nsol[iv], -cnt[iv], -len(iv))):
        if npitches(iv) <= 3 or is_scale_run(iv) or any(related(iv, k) for k in kept + longs):
            continue
        longs.append(iv)
        if len(longs) == 2:
            break

    info = {m: (t, y, k, tp) for m, t, y, k, tp in solos}
    P("#" * 74)
    P("%s   (%d solos, %d notes; years %s)" % (label.upper(), len(solos), nnotes,
                                               ", ".join(sorted({str(s[2]) for s in solos}))))
    P("solos: " + "; ".join("%s (%d, %d bpm)" % (t, y, round(tp or 0)) for m, t, y, k, tp in solos))
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
            for k in range(st + 1, st + L):
                if cidx[k] is not None and cidx[k] != cidx[st] and csym[k] != csym[st]:
                    nxt = (chords[k], csym[k]); break
            fn = function(ch, nxt[0], key_pc_of(info[melid][2]))
            details.append((melid, st, first_deg, fn))
            if not main:
                continue
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
            barstart = lastn["pos"] - (lastn["beat"] - 1) - lastn["frac"]
            tb = next(b for b in sorted(barstart + k * num + sb for k in range(3) for sb in strong) if b >= lastn["pos"])
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
        pitches = [n["pitch"] for n in seg]
        degs = [degree((p - ch[0]) % 12, ch) if ch else "?" for p in pitches]
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
        t, y, key, tempo = info[melid]
        P("")
        P("%s  intervals [%s]  (%d notes)  occurrences %d in %d solos  contour %s" % (
            rank, " ".join("%+d" % x for x in iv), L, cnt[iv], nsol[iv], contour(iv)))
        P("  notes (as played): " + " ".join(nname(p) for p in pitches))
        P("  degrees:           " + " ".join(degs))
        P("  grid starts:       " + " ".join(fmt_beats(r) for r in rel))
        P("  metric pos:        " + " ".join(metpos(n) for n in seg) + "   (bar %d)" % seg[0]["bar"])
        P("  played onsets (s): " + " ".join("%.3f" % (n["onset"] - seg[0]["onset"]) for n in seg))
        P("  played durs (s):   " + " ".join("%.3f" % n["dur"] for n in seg) +
          "   (beat = %.3f s)" % seg[0]["bd"])
        P("  chords: %s%s   function: %s" % (" | ".join(syms) or "none",
                                            ("  (next: %s)" % nx[1]) if nx and nx[1] not in syms else "", fn))
        P("  from: %s (%d), %d bpm, key %s, at %.2f s into the solo" % (t, y, round(tempo or 0), key or "?", seg[0]["onset"]))
        P("  MML (as played): " + mml(pitches, ioi))
        if main:
            P("  first-note degree across occurrences: " + ", ".join("%s x%d" % kv for kv in degc.most_common(4)) +
              "; functions: " + ", ".join("%s x%d" % kv for kv in fc.most_common(3)))

    tot = sum(S_len.values()) or 1
    P("")
    P("-- %s summary (over all %d occurrences of the %d main licks) --" % (label, tot, len(kept)))
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
