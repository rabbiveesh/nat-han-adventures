"""Tommy Bryant on 'On the Sunny Side of the Street' (Sonny Side Up, 1957): paired quarters
(beat 1 = beat 2, beat 3 = beat 4 by pitch) vs walking, from the Demucs bass stem of a lossy copy."""
import sys, statistics as st
from collections import Counter
import numpy as np
import librosa, torch, torchcrepe
from madmom.features.downbeats import RNNDownBeatProcessor, DBNDownBeatTrackingProcessor

MIX, BASS, VOC = sys.argv[1:4]
SR, HOP = 22050, 128
NAMES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
y, _ = librosa.load(BASS, sr=SR, mono=True)
rms = librosa.feature.rms(y=y, frame_length=1024, hop_length=HOP)[0]
env = librosa.onset.onset_strength(y=y, sr=SR, hop_length=HOP, lag=2, max_size=3)
beats = DBNDownBeatTrackingProcessor(beats_per_bar=[4], fps=100, min_bpm=100, max_bpm=140)(RNNDownBeatProcessor()(MIX))
bt, bn = beats[:, 0], beats[:, 1].astype(int)
tempo = 60 / np.median(np.diff(bt))
y16 = librosa.resample(y, orig_sr=SR, target_sr=16000)
f0, per = torchcrepe.predict(torch.tensor(y16)[None], 16000, 160, fmin=35, fmax=400, model="full",
                             decoder=torchcrepe.decode.viterbi, return_periodicity=True, batch_size=2048,
                             device="cuda" if torch.cuda.is_available() else "cpu")
f0, per = f0[0].cpu().numpy(), per[0].cpu().numpy()
v, _ = librosa.load(VOC, sr=SR, mono=True)
vr = librosa.feature.rms(y=v, frame_length=4096, hop_length=4096)[0]
fr = lambda t: int(t * SR / HOP)


def pitch(t, span=0.25):
    i = int(t * 100)
    seg = f0[i + 4:i + int(span * 100)][per[i + 4:i + int(span * 100)] > 0.3]
    return int(round(69 + 12 * np.log2(np.median(seg) / 440))) if len(seg) >= 3 else None


def attack(t, L):
    """(is there a (re)attack near beat t, peak level dB, how long it rings as a share of the beat)"""
    a, b = fr(t - 0.12 * L), fr(t + 0.15 * L)
    pre = rms[max(0, fr(t - 0.2 * L)):fr(t - 0.02 * L)]
    post = rms[a:b]
    if not len(post):
        return False, -99, 0
    pk = post.max()
    rise = pk / (pre.min() + 1e-9) if len(pre) else 10
    strong = env[a:b].max() > np.percentile(env, 70)
    end = fr(t + L)
    j = a + int(np.argmax(post))
    k = j
    while k < min(end, len(rms)) and rms[k] > pk * 10 ** (-12 / 20):
        k += 1
    return (rise > 1.4 or strong), 20 * np.log10(pk + 1e-9), (k - fr(t)) * HOP / SR / L


def build(phase):
    out = []
    for k in range(len(bt) - 4):
        if (bn[k] - 1 - phase) % 4 != 0:
            continue
        out.append(k)
    return out
# madmom's downbeat can sit a beat off; choose the bar phase where beats pair up (1=2, 3=4) most often
def pairscore(ks):
    sc = 0
    for k in ks:
        ps = [pitch(bt[k + j]) for j in range(4)]
        sc += (ps[0] is not None and ps[0] == ps[1]) + (ps[2] is not None and ps[2] == ps[3])
    return sc
phase_scores = {ph: pairscore(build(ph)) for ph in range(4)}
PHASE = int(sys.argv[4]) if len(sys.argv) > 4 else max(phase_scores, key=phase_scores.get)
bars = []
for k in build(PHASE):
    b = bt[k:k + 5]
    L = float(np.mean(np.diff(b)))
    at = [attack(b[j], L) for j in range(4)]
    ps = [pitch(b[j]) for j in range(4)]
    voc = vr[int(b[0] * SR / 4096):int(b[4] * SR / 4096)].mean() if b[4] * SR / 4096 < len(vr) else 0
    # extra onsets late in a beat (pickups / skips)
    extra = []
    for j in range(4):
        lo, hi = b[j] + 0.3 * L, b[j + 1] - 0.12 * L
        seg = env[fr(lo):fr(hi)]
        if len(seg) and seg.max() > np.percentile(env, 90):
            extra.append((j + 1, (lo - b[j]) / L + np.argmax(seg) * HOP / SR / L))
    bars.append(dict(t=b[0], b=b, L=L, at=at, ps=ps, voc=voc, extra=extra))


def kind(x):
    p, a = x["ps"], [q[0] for q in x["at"]]
    if None in p:
        return "?"
    s1, s2 = p[0] == p[1], p[2] == p[3]
    if s1 and s2 and a[1] and a[3]:
        return "P"      # paired quarters, beats 2 and 4 re-struck
    if s1 and s2:
        return "H"      # pairs without clear re-attack on 2/4: half notes (or very soft repeats)
    if (s1 and a[1]) or (s2 and a[3]):
        return "p"      # one half paired
    if len(set(p)) == 4 or (len(set(p)) >= 3 and not s1 and not s2):
        return "W"
    return "o"


for x in bars:
    x["k"] = kind(x)
C = Counter(x["k"] for x in bars)
n = len(bars)

# form: chorus length 32, phase from self-similarity (A sections alike, bridge different)
def pcvec(x):
    v = np.zeros(12)
    for p in x["ps"]:
        if p is not None:
            v[p % 12] += 1
    return v / (np.linalg.norm(v) + 1e-9)
V = np.array([pcvec(x) for x in bars])
def sim(i, j):
    return float(V[i] @ V[j]) if 0 <= i < n and 0 <= j < n else np.nan
lag32 = np.nanmean([sim(i, i + 32) for i in range(n - 32)])
lag8 = np.nanmean([sim(i, i + 8) for i in range(n - 8)])
lag24 = np.nanmean([sim(i, i + 24) for i in range(n - 24)])
best = None
for o in range(32):
    a, bb = [], []
    for c0 in range(o, n - 31, 32):
        for i in range(8):
            a += [sim(c0 + i, c0 + 8 + i), sim(c0 + 8 + i, c0 + 24 + i)]
            bb += [sim(c0 + i, c0 + 16 + i), sim(c0 + 8 + i, c0 + 16 + i)]
    sc = np.nanmean(a) - np.nanmean(bb)
    if best is None or sc > best[0]:
        best = (sc, o)
off = best[1]
# better: find the tune's opening (C | E7 | F | F in C major) from the bass note on beat 1
roots = [(x["ps"][0] % 12) if x["ps"][0] is not None else -1 for x in bars]
starts = [i for i in range(n - 3) if roots[i] == 0 and roots[i + 1] == 4 and roots[i + 2] == 5]
if starts:
    # choose the phase mod 32 that most of these openings agree on
    off = Counter(i % 32 for i in starts).most_common(1)[0][0]
    while off - 32 >= 0:
        off -= 32
OPENINGS = starts
sect = lambda i: ("intro" if i < off else "ABAB"[0] if False else
                  ["A1", "A2", "B", "A3"][((i - off) % 32) // 8])

L = []
P = L.append
P("")
P("####################################################################################################")
P("TOMMY BRYANT, 'ON THE SUNNY SIDE OF THE STREET' -- Sonny Side Up (Verve, 1957), PAIRED QUARTERS")
P("(Demucs htdemucs bass stem of a lossy copy, AAC ~200 kbps; madmom beats; CREPE pitch per beat. Replaces")
P(" the first attempt above, which ran on a double-time 240 bpm grid and mistook quarters for half notes.)")
P("####################################################################################################")
P("tempo %.0f bpm (beat %.3f s), 4/4; %d bars from %.1f s to %.1f s" % (tempo, 60 / tempo, n, bars[0]["t"], bars[-1]["b"][4]))
P("bar lines: madmom's downbeat shifted by %d beat(s) so that pairs fall on 1-2 and 3-4 (pairing score per phase: %s)" % (
    PHASE, phase_scores))
P("form: 32-bar AABA (similarity at a 32-bar lag %.2f vs 8-bar %.2f, 24-bar %.2f); chorus 1 starts at bar %d (%.1f s);" % (
    lag32, lag8, lag24, off + 1, bars[off]["t"]))
P("      %d full choruses + %d bars; bars where the bass plays C | E | F (the tune's opening, C major): %s" % (
    (n - off) // 32, (n - off) % 32, ", ".join(str(i + 1) for i in OPENINGS)))
P("bar types: P paired quarters (1=2 and 3=4 by pitch, 2 and 4 re-struck) %d (%.0f%%); p one half paired %d (%.0f%%);" % (
    C["P"], 100 * C["P"] / n, C["p"], 100 * C["p"] / n))
P("           H pairs with no clear re-attack on 2/4 (half notes, or very soft repeats) %d (%.0f%%); W walking (3-4 different" % (
    C["H"], 100 * C["H"] / n))
P("           pitches) %d (%.0f%%); o other %d; ? pitch unclear %d" % (C["W"], 100 * C["W"] / n, C["o"], C["?"]))
P("timeline (rows = 8 bars, labelled by AABA section; v = a bar with voice/horn melody in the vocals stem):")
for i in range(0, n, 8):
    row = bars[i:i + 8]
    P("  %6.1fs bar %3d %-5s %s   %s" % (row[0]["t"], i + 1, sect(i), " ".join(x["k"] for x in row),
                                       "".join("v" if x["voc"] > np.percentile(vr, 60) else "." for x in row)))
# section breakdown
P("by section (share of bars that are P / p / W):")
for s in ("intro", "A1", "A2", "B", "A3"):
    xs = [bars[i]["k"] for i in range(n) if sect(i) == s]
    if xs:
        cc = Counter(xs)
        P("  %-5s %3d bars: P %.0f%%  p %.0f%%  H %.0f%%  W %.0f%%" % (s, len(xs), 100 * cc["P"] / len(xs), 100 * cc["p"] / len(xs),
                                                                100 * cc["H"] / len(xs), 100 * cc["W"] / len(xs)))
# what the pairs are
mv, rel = Counter(), Counter()
lv2, lv4, ln = [], [], {1: [], 2: [], 3: [], 4: []}
regs = []
for x in bars:
    p = x["ps"]
    regs += [q for q in p if q]
    if x["k"] != "P":
        continue
    d = (p[2] - p[0]) % 12
    mv[{0: "same" if p[2] == p[0] else "octave", 7: "up a 5th / down a 4th", 5: "up a 4th / down a 5th", 1: "half step up",
        11: "half step down", 2: "whole step up", 10: "whole step down", 3: "minor 3rd up", 9: "minor 3rd down",
        4: "major 3rd up", 8: "major 3rd down"}.get(d, "tritone")] += 1
    a = x["at"]
    lv2.append(a[1][1] - a[0][1])
    lv4.append(a[3][1] - a[2][1])
    for j in range(4):
        ln[j + 1].append(a[j][2])
nP = max(1, C["P"])
P("in the %d paired-quarter bars:" % C["P"])
P("  move from pair 1 (beats 1-2) to pair 2 (beats 3-4): " + ", ".join("%s %.0f%%" % (k, 100 * v / nP) for k, v in mv.most_common()))
P("  (no chord data: 'up a 4th / down a 5th' and 'up a 5th / down a 4th' = the classic root-to-5th or 5th-to-root move)")
if lv2:
    P("  accent: beat 2 vs beat 1 %+.1f dB, beat 4 vs beat 3 %+.1f dB (median)" % (st.median(lv2), st.median(lv4)))
    P("  ring length as a share of the beat: beat 1 %.0f%%, beat 2 %.0f%%, beat 3 %.0f%%, beat 4 %.0f%% (median)" % tuple(
        100 * st.median(ln[j]) for j in (1, 2, 3, 4)))
# how he leaves the pattern
leave = Counter()
for i in range(1, n):
    if bars[i - 1]["k"] == "P" and bars[i]["k"] != "P":
        x = bars[i]
        if x["k"] == "W":
            leave["a walking bar"] += 1
        elif x["k"] == "p":
            leave["half a bar walked (one pair kept)"] += 1
        else:
            leave["other (%s)" % x["k"]] += 1
    if bars[i]["k"] == "P":
        for beat, pos in bars[i]["extra"]:
            leave["skip/pickup inside a paired bar, late in beat %d" % beat] += 1
P("  ways out of the pattern / decorations: " + ", ".join("%s %d" % kv for kv in leave.most_common()))
# pickups and anticipations overall
pk = Counter()
ant = Counter()
for i, x in enumerate(bars):
    for beat, pos in x["extra"]:
        cls = "triplet 2nd (~0.33)" if pos < 0.42 else "straight 8th (~0.5)" if pos < 0.6 else "swung 8th / last triplet (~0.67)" if pos < 0.78 else "late (~0.8)"
        pk["late in beat %d: %s" % (beat, cls)] += 1
        nxt_attacked = x["at"][beat][0] if beat < 4 else (bars[i + 1]["at"][0][0] if i + 1 < n else True)
        if not nxt_attacked:
            ant["on the 'and' of %d, held over the next beat" % beat] += 1
P("pickups/skips (extra notes late in a beat), per bar: %.2f; " % (sum(pk.values()) / n) + ", ".join("%s %d" % kv for kv in pk.most_common(8)))
P("anticipations (a late-beat note held over the next beat, no new attack there): %d in %d bars: " % (sum(ant.values()), n) +
  ", ".join("%s %d" % kv for kv in ant.most_common()))
q = np.percentile(regs, [10, 50, 90])
P("register: median %s%d (MIDI %d), 10th %d (%s%d), 90th %d (%s%d)" % (
    NAMES[int(q[1]) % 12], int(q[1]) // 12 - 1, q[1], q[0], NAMES[int(q[0]) % 12], int(q[0]) // 12 - 1,
    q[2], NAMES[int(q[2]) % 12], int(q[2]) // 12 - 1))
pcs = Counter(p % 12 for p in regs)
P("commonest bass pitch classes (key hint): " + " ".join("%s %d" % (NAMES[k], v) for k, v in pcs.most_common(6)))
print("\n".join(L))
import json
json.dump([dict(t=float(x["t"]), end=float(x["b"][4]), kind=x["k"], pitches=x["ps"]) for x in bars],
          open("pairs_bars.json", "w"), indent=0)
