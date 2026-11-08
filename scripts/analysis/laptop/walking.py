import sqlite3, re, sys, statistics as st
from collections import Counter, defaultdict

c = sqlite3.connect(sys.argv[1])
PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def year_of(s):
    m = re.findall(r"(1[89]\d\d|20\d\d)", s or "")
    return int(m[-1]) if m else None


def chord(sym):
    if not sym or sym == "NC":
        return None
    m = re.match(r"([A-G])([b#]?)(.*)", sym.split("/")[0])
    if not m:
        return None
    root = (PC[m.group(1)] + {"": 0, "b": -1, "#": 1}[m.group(2)]) % 12
    q = m.group(3)
    for pre, k in (("m7b5", "half"), ("o", "dim"), ("-", "min"), ("+", "aug"), ("sus", "sus"), ("j7", "maj"), ("7", "dom"), ("6", "maj")):
        if q.startswith(pre):
            return root, k
    return root, "maj"


TONES = {"maj": {0, 4, 7, 11, 9}, "dom": {0, 4, 7, 10}, "min": {0, 3, 7, 10, 9}, "half": {0, 3, 6, 10},
         "dim": {0, 3, 6, 9}, "aug": {0, 4, 8, 10}, "sus": {0, 5, 7, 10}}
SCALE = {"maj": {0, 2, 4, 5, 7, 9, 11}, "dom": {0, 2, 4, 5, 7, 9, 10}, "min": {0, 2, 3, 5, 7, 9, 10},
         "half": {0, 1, 3, 5, 6, 8, 10}, "dim": {0, 2, 3, 5, 6, 8, 9, 11}, "aug": {0, 2, 4, 6, 8, 10},
         "sus": {0, 2, 4, 5, 7, 9, 10}}


def bassists(lineup):
    out = []
    for part in (lineup or "").split(";"):
        m = re.match(r"\s*(.+?)\s*\((b|bass|db)\)\s*$", part.strip())   # (bs) = baritone sax
        if m:
            name = m.group(1).strip().title().replace("Curley", "Curly").replace("Dougwatkins", "Doug Watkins")
            name = re.sub(r"Russel$", "Russell", name)
            out.append(name)
    return out


solos = {}
for melid, perf, rd, feel, sig, lineup in c.execute(
        "select s.melid,s.performer,t.recordingdate,s.rhythmfeel,s.signature,t.lineup from solo_info s join track_info t using(trackid)"):
    solos[melid] = dict(perf=perf, year=year_of(rd), feel=feel or "", sig=sig or "", bass=bassists(lineup))


def beats_of(melid):
    rows = list(c.execute("select bar,beat,chord,bass_pitch,signature from beats where melid=? order by onset", (melid,)))
    out, cur, sig = [], None, "4/4"
    for bar, beat, ch, bp, s in rows:
        if s:
            sig = s
        new = False
        if ch:
            cc = chord(ch)
            new = cc is not None and (cur is None or cc != cur)
            cur = cc if cc else cur
        out.append(dict(bp=bp if bp and bp > 0 else None, ch=cur, change=new, sig=sig))
    return out


def stats(melids):
    C = Counter()
    P, runs = [], []
    nsolo = 0
    for m in melids:
        bs = beats_of(m)
        if not any(b["bp"] for b in bs):
            continue
        nsolo += 1
        # intervals, runs, pedals
        run_len, run_dir = 0, 0
        same_pc = 0
        for i, b in enumerate(bs):
            if not b["bp"] or b["sig"] != "4/4":
                if run_len:
                    runs.append(run_len)
                run_len, run_dir = 0, 0
                if same_pc >= 4:
                    C["pedal"] += same_pc
                same_pc = 0
                continue
            C["beats"] += 1
            P.append(b["bp"])
            ch = b["ch"]
            rel = (b["bp"] - ch[0]) % 12 if ch else None
            if b["change"] and ch:
                C["chg"] += 1
                C["chg_" + {0: "root", 3: "3rd", 4: "3rd", 7: "5th"}.get(rel, "other")] += 1
            elif ch:
                C["in"] += 1
                C["in_" + ("chord" if rel in TONES[ch[1]] else "scale" if rel in SCALE[ch[1]] else "chromatic")] += 1
            nb = bs[i + 1] if i + 1 < len(bs) else None
            if nb and nb["change"] and nb["ch"]:
                d = (nb["ch"][0] - b["bp"]) % 12
                C["app"] += 1
                C["app_" + {1: "half below", 11: "half above", 2: "whole", 10: "whole", 5: "fifth (V-I)", 0: "same note"}.get(d, "other")] += 1
            prev = bs[i - 1] if i else None
            if prev and prev["bp"] and prev["sig"] == "4/4":
                iv = b["bp"] - prev["bp"]
                a = abs(iv)
                C["iv"] += 1
                C["iv_" + ("0" if a == 0 else "1" if a == 1 else "2" if a == 2 else "3-4" if a <= 4 else "5-7" if a <= 7 else ">7")] += 1
                C["up"] += iv > 0
                C["down"] += iv < 0
                if 1 <= a <= 2:
                    dr = 1 if iv > 0 else -1
                    if run_dir == dr:
                        run_len += 1
                    else:
                        if run_len:
                            runs.append(run_len)
                        run_len, run_dir = 1, dr
                else:
                    if run_len:
                        runs.append(run_len)
                    run_len, run_dir = 0, 0
                if prev["bp"] % 12 == b["bp"] % 12:
                    same_pc += 1
                else:
                    if same_pc >= 4:
                        C["pedal"] += same_pc
                    same_pc = 1
            else:
                same_pc = 1
        if same_pc >= 4:
            C["pedal"] += same_pc
        if run_len:
            runs.append(run_len)
    return C, P, runs, nsolo


def fmt(label, C, P, runs, nsolo, out):
    if not C["beats"]:
        out.append("%-34s no bass data" % label)
        return None
    pct = lambda k, n: 100 * C[k] / max(1, C[n])
    q = st.quantiles(P, n=10)
    row = dict(label=label, n=C["beats"], solos=nsolo, med=st.median(P), p10=q[0], p90=q[-1],
               rep=pct("iv_0", "iv"), half=pct("iv_1", "iv"), whole=pct("iv_2", "iv"), third=pct("iv_3-4", "iv"),
               fourth=pct("iv_5-7", "iv"), big=pct("iv_>7", "iv"), up=pct("up", "iv"), down=pct("down", "iv"),
               root=pct("chg_root", "chg"), third_c=pct("chg_3rd", "chg"), fifth=pct("chg_5th", "chg"), other=pct("chg_other", "chg"),
               hb=pct("app_half below", "app"), ha=pct("app_half above", "app"), wh=pct("app_whole", "app"),
               v=pct("app_fifth (V-I)", "app"), same=pct("app_same note", "app"), aoth=pct("app_other", "app"),
               ct=pct("in_chord", "in"), sc=pct("in_scale", "in"), chrom=pct("in_chromatic", "in"),
               run=st.mean(runs) if runs else 0, pedal=100 * C["pedal"] / C["beats"])
    out.append("")
    out.append("== %s ==   %d beats with bass, %d solos" % (label, row["n"], nsolo))
    out.append("  register (MIDI): median %.0f, 10th %.0f, 90th %.0f" % (row["med"], row["p10"], row["p90"]))
    out.append("  intervals beat to beat: repeat %.1f%%  half step %.1f%%  whole step %.1f%%  3rds %.1f%%  4ths/5ths %.1f%%  > 5th %.1f%%   up %.0f%% / down %.0f%%" % (
        row["rep"], row["half"], row["whole"], row["third"], row["fourth"], row["big"], row["up"], row["down"]))
    out.append("  on a chord change: root %.0f%%  3rd %.0f%%  5th %.0f%%  other %.0f%%   (n=%d)" % (
        row["root"], row["third_c"], row["fifth"], row["other"], C["chg"]))
    out.append("  beat before a change -> next root: half step below %.0f%%  half above %.0f%%  whole step %.0f%%  fifth/V-I %.0f%%  same note %.0f%%  other %.0f%%" % (
        row["hb"], row["ha"], row["wh"], row["v"], row["same"], row["aoth"]))
    out.append("  beats inside a chord: chord tones %.0f%%  scale non-chord %.0f%%  chromatic %.0f%%" % (row["ct"], row["sc"], row["chrom"]))
    out.append("  stepwise runs (one direction): mean %.2f steps   pedal (4+ beats same pitch class): %.1f%% of beats" % (row["run"], row["pedal"]))
    return row


GROUPS = [
    ("Miles Davis 1951-59", lambda s: s["perf"] == "Miles Davis" and s["year"] and 1951 <= s["year"] <= 1959),
    ("John Coltrane 1960-64", lambda s: s["perf"] == "John Coltrane" and s["year"] and 1960 <= s["year"] <= 1964),
    ("Charlie Parker 1945-51", lambda s: s["perf"] == "Charlie Parker"),
    ("Sonny Rollins 1954-72", lambda s: s["perf"] == "Sonny Rollins"),
    ("Wayne Shorter 1961-66", lambda s: s["perf"] == "Wayne Shorter"),
    ("Louis Armstrong 1925-28", lambda s: s["perf"] == "Louis Armstrong"),
    ("whole database", lambda s: True),
]
swing = lambda s: "SWING" in s["feel"] and s["sig"] in ("4/4", "")
out, rows = [], []
out.append("WALKING BASS BY SOLOIST AND BY BASSIST -- WJazzD beats.bass_pitch (one pitch per beat)")
out.append("=" * 100)
out.append("Only swing-feel solos in 4/4, only beats that have a bass pitch. The bass pitches are the database's")
out.append("beat-wise bass estimates: about 120k beats, mostly automatic (Abesser et al.'s walking-bass method;")
out.append("41 solos were also checked by hand), so expect a few percent of octave or neighbour-note errors.")
out.append("Chords are the DB's annotated changes; 'chord change' = a beat whose chord differs from the previous one.")
for label, f in GROUPS:
    ms = [m for m, s in solos.items() if f(s) and swing(s)]
    C, P, runs, n = stats(ms)
    r = fmt(label, C, P, runs, n, out)
    if r:
        rows.append(r)
# bassists
by = defaultdict(list)
for m, s in solos.items():
    if swing(s):
        for b in s["bass"]:
            by[b].append(m)
out.append("")
out.append("#" * 100)
out.append("BY BASSIST (from track_info.lineup; bassists with >= 4 swing solos)")
brow = []
for b, ms in sorted(by.items(), key=lambda kv: -len(kv[1])):
    if len(ms) < 4:
        continue
    C, P, runs, n = stats(ms)
    r = fmt("%s (behind: %s)" % (b, ", ".join(sorted({solos[m]["perf"] for m in ms}))[:120]), C, P, runs, n, out)
    if r:
        r["label"] = b
        brow.append(r)
summ = ["SUMMARY TABLE", "%-26s %6s %4s %4s %5s %5s %5s %5s %5s %5s %5s %5s %5s %5s" % (
    "group / bassist", "beats", "reg", "rep", "half", "whole", "3rds", "4/5", "root@", "hb", "ha", "V-I", "chrom", "pedal")]
for r in rows + brow:
    summ.append("%-26s %6d %4.0f %4.1f %5.1f %5.1f %5.1f %5.1f %5.0f %5.0f %5.0f %5.0f %5.0f %5.1f" % (
        r["label"][:26], r["n"], r["med"], r["rep"], r["half"], r["whole"], r["third"], r["fourth"], r["root"],
        r["hb"], r["ha"], r["v"], r["chrom"], r["pedal"]))
summ.append("(reg = median MIDI; rep/half/whole/3rds/4/5 = beat-to-beat intervals %; root@ = root on chord changes %;")
summ.append(" hb/ha/V-I = how the beat before a change approaches the next root %; chrom = chromatic tones inside chords %;")
summ.append(" pedal = % of beats in 4+ beat same-pitch-class pedals)")
print("\n".join(summ + [""] + out))
