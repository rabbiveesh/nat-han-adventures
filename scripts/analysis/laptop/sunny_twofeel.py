"""Tommy Bryant's two-feel on "On the Sunny Side of the Street" (Sonny Side Up, 1957), from the
Demucs (htdemucs) bass stem of a lossy copy.  Rough by nature; statistics only."""
import sys, statistics as st
from collections import Counter
import numpy as np
import librosa, torch, torchcrepe
from madmom.features.downbeats import RNNDownBeatProcessor, DBNDownBeatTrackingProcessor

MIX, BASS, VOC = sys.argv[1], sys.argv[2], sys.argv[3]
SR, HOP = 22050, 256
NAMES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
y, _ = librosa.load(BASS, sr=SR, mono=True)
env = librosa.onset.onset_strength(y=y, sr=SR, hop_length=HOP, lag=2, max_size=3)
on = np.array(librosa.onset.onset_detect(onset_envelope=env, sr=SR, hop_length=HOP, delta=0.07, wait=3, units="time")) - 0.015
ioi = np.diff(on); ioi = ioi[(ioi > 0.2) & (ioi < 1.2)]
act = RNNDownBeatProcessor()(MIX)
cands = []
for lo, hi in ((60, 120), (100, 200), (180, 300)):
    b = DBNDownBeatTrackingProcessor(beats_per_bar=[4], fps=100, min_bpm=lo, max_bpm=hi)(act)
    cands.append(b)
# prefer the grid where the bass's notes mostly sit on beats (walking = every beat, two = every other)
def fit(b):
    bt = b[:, 0]
    d = [np.min(np.abs(bt - t)) for t in on]
    return np.median(d) / np.median(np.diff(bt))
beats = min(cands, key=lambda b: (fit(b) > 0.12, -len(b)))
bt, bn = beats[:, 0], beats[:, 1].astype(int)
tempo = 60 / np.median(np.diff(bt))

y16 = librosa.resample(y, orig_sr=SR, target_sr=16000)
f0, per = torchcrepe.predict(torch.tensor(y16)[None], 16000, 160, fmin=35, fmax=400, model="full",
                             decoder=torchcrepe.decode.viterbi, return_periodicity=True, batch_size=2048,
                             device="cuda" if torch.cuda.is_available() else "cpu")
f0, per = f0[0].cpu().numpy(), per[0].cpu().numpy()
rms = librosa.feature.rms(y=y, frame_length=1024, hop_length=HOP)[0]
v, _ = librosa.load(VOC, sr=SR, mono=True)
vrms = librosa.feature.rms(y=v, frame_length=4096, hop_length=4096)[0]


def pitch(t):
    i = int(t * 100)
    seg = f0[i + 3:i + 12][per[i + 3:i + 12] > 0.3]
    return int(round(69 + 12 * np.log2(np.median(seg) / 440))) if len(seg) else None


def lvl(t):
    i = int(t * SR / HOP)
    return 20 * np.log10(rms[i:i + 6].max() + 1e-9) if i < len(rms) else -99


def length(t, nxt):
    """seconds until the note drops 15 dB below its peak, or the next onset"""
    i = int(t * SR / HOP)
    pk = rms[i:i + 6].max() + 1e-9
    j = i + 3
    end = int(nxt * SR / HOP) if nxt else len(rms)
    while j < min(end, len(rms)) and rms[j] > pk * 10 ** (-15 / 20):
        j += 1
    return (j - i) * HOP / SR


def onsets_in(a, b):
    return [t for t in on if a <= t < b]


bars = []
down = [k for k in range(len(bt)) if bn[k] == 1 and k + 4 < len(bt)]
for k in down:
    b = bt[k:k + 5]
    L = np.diff(b)
    near = [any(abs(t - b[j]) < 0.18 * L[min(j, 3)] for t in on) for j in range(4)]
    feel = "T" if near[0] and near[2] and not near[1] and not near[3] else \
           "W" if all(near) else "w" if sum(near) == 3 else "." if sum(near) <= 1 else "?"
    vocal = np.mean(vrms[int(b[0] * SR / 4096):int(b[4] * SR / 4096)]) if b[4] * SR / 4096 < len(vrms) else 0
    bars.append(dict(k=k, t=b[0], b=b, L=L, near=near, feel=feel, vocal=vocal))

C = Counter(x["feel"] for x in bars)
vthr = np.percentile([x["vocal"] for x in bars], 60)
L = []
P = L.append
P("")
P("####################################################################################################")
P("TOMMY BRYANT, TWO-FEEL -- 'On the Sunny Side of the Street', Sonny Side Up (Verve, 1957)")
P("(Demucs htdemucs bass stem of a lossy copy, AAC ~200 kbps; madmom beats; CREPE pitch; rough)")
P("####################################################################################################")
P("tempo %.0f bpm (beat %.3f s); %d bars found; bars: two-feel T %d (%.0f%%), walking W %d (%.0f%%), 3 of 4 beats w %d, "
  "sparse . %d, other ? %d" % (tempo, 60 / tempo, len(bars), C["T"], 100 * C["T"] / len(bars), C["W"], 100 * C["W"] / len(bars),
                               C["w"], C["."], C["?"]))
P("timeline, 8 bars per row (T two, W walk, w walk with one gap, . sparse, ? other; v = a voice/horn-melody bar")
P("from the vocals stem):")
for i in range(0, len(bars), 8):
    row = bars[i:i + 8]
    P("  %5.1fs  bar %3d  %s   %s" % (row[0]["t"], i + 1, " ".join(x["feel"] for x in row),
                                    "".join("v" if x["vocal"] > vthr else "." for x in row)))

# what he plays on 1 and 3 in two-feel bars
rel = Counter()
reg, lens, pick, antic, ghosts, total = [], [], Counter(), Counter(), 0, 0
for x in bars:
    if x["feel"] != "T":
        continue
    b, Lb = x["b"], x["L"]
    n1 = [t for t in on if abs(t - b[0]) < 0.18 * Lb[0]]
    n3 = [t for t in on if abs(t - b[2]) < 0.18 * Lb[2]]
    if n1 and n3:
        p1, p3 = pitch(n1[0]), pitch(n3[0])
        if p1 and p3:
            d = (p3 - p1) % 12
            rel[{0: "same note (root-root?)" if p3 == p1 else "octave", 7: "up a 5th / down a 4th", 5: "up a 4th / down a 5th",
                 }.get(d, "step (1-2)" if d in (1, 2, 10, 11) else "3rd (3-4)" if d in (3, 4, 8, 9) else "other")] += 1
            reg += [p1, p3]
        for t, nxt_beat in ((n1[0], b[2]), (n3[0], b[4])):
            later = [u for u in on if t + 0.1 < u]
            ln = length(t, later[0] if later else None)
            lens.append(ln / (2 * np.mean(Lb)))
    # pickups into 3 (in beat 2) and into the next 1 (in beat 4)
    for beat_i, into in ((1, "3"), (3, "1")):
        a, e = b[beat_i], b[beat_i + 1]
        for t in onsets_in(a + 0.25 * (e - a), e - 0.12 * (e - a)):
            f = (t - a) / (e - a)
            cls = "triplet 2nd (~0.33)" if f < 0.42 else "straight 8th (~0.5)" if f < 0.6 else \
                  "swung 8th / last triplet (~0.67)" if f < 0.78 else "late 16th (~0.8)"
            pick["into %s: %s" % (into, cls)] += 1
            # anticipation: the pickup is held over the next beat (no onset there) and is a new pitch
            nxt = b[beat_i + 1]
            if not any(abs(u - nxt) < 0.18 * (e - a) for u in on):
                antic["on the 'and' of %d, held over beat %s" % (beat_i + 1, into)] += 1
    for t in onsets_in(b[0] - 0.1 * Lb[0], b[4] - 0.1 * Lb[3]):
        total += 1
        i = int(t * 100)
        clear = per[i + 3:i + 10].mean() if i + 10 < len(per) else 0
        if clear < 0.3:
            ghosts += 1
nT = C["T"]
P("")
P("in the %d two-feel bars:" % nT)
P("  beat 3 relative to beat 1 (no chord data: interval between the two notes): " +
  ", ".join("%s %.0f%%" % (k, 100 * v / max(1, sum(rel.values()))) for k, v in rel.most_common()))
P("  pickups (extra notes late in beat 2 or 4), per two-feel bar: %.2f; where they land: " % (sum(pick.values()) / max(1, nT)) +
  ", ".join("%s %d" % kv for kv in pick.most_common()))
P("  anticipations (a pickup held over the next main beat): %d in %d bars: " % (sum(antic.values()), nT) +
  ", ".join("%s %d" % kv for kv in antic.most_common()))
P("  ghost/dead (onsets with no clear pitch): %.0f%% of onsets (rip + bleed: an upper bound)" % (100 * ghosts / max(1, total)))
if lens:
    P("  note length of the half notes on 1 and 3: median %.0f%% of two beats (25th %.0f%%, 75th %.0f%%) -> %s" % (
        100 * st.median(lens), 100 * np.percentile(lens, 25), 100 * np.percentile(lens, 75),
        "legato, held nearly full" if st.median(lens) > 0.75 else "medium" if st.median(lens) > 0.5 else "short, detached"))
if reg:
    P("  register: median %s%d (MIDI %d), 10th %d, 90th %d" % (NAMES[int(st.median(reg)) % 12], int(st.median(reg)) // 12 - 1,
                                                          int(st.median(reg)), np.percentile(reg, 10), np.percentile(reg, 90)))
print("\n".join(L))
