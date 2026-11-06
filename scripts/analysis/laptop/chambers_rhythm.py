"""Paul Chambers' rhythm inside the beat, from Demucs (htdemucs, 4-stem) bass stems.

usage: chambers_rhythm.py sep4/htdemucs "TRACK DIR NAMES..." (mix files are found by name)
Beats: madmom downbeat tracker on the full mix, tempo range set from the bass's own onset rate.
Onsets: librosa onset detection on the bass stem. Per beat: the main note near the beat, plus any extra
onsets later in the beat (skip notes); rakes = 2+ extra onsets in one beat; ghost/dead = an onset with
no clear pitch (CREPE periodicity) or 15 dB+ quieter than the beat's main note.
"""
import os, sys, glob, statistics as st
from collections import Counter
import numpy as np
import librosa, torch, torchcrepe
from madmom.features.downbeats import RNNDownBeatProcessor, DBNDownBeatTrackingProcessor

ROOT, MIXDIRS = sys.argv[1], sys.argv[2:]
SR = 22050
HOP = 256


def mix_for(track):
    for d in MIXDIRS:
        for f in glob.glob(os.path.join(d, "*.flac")):
            if os.path.splitext(os.path.basename(f))[0] == track:
                return f


def analyse(track):
    y, _ = librosa.load(os.path.join(ROOT, track, "bass.wav"), sr=SR, mono=True)
    env = librosa.onset.onset_strength(y=y, sr=SR, hop_length=HOP, lag=2, max_size=3)
    on = librosa.onset.onset_detect(onset_envelope=env, sr=SR, hop_length=HOP, backtrack=False, delta=0.07, wait=3, units="time")
    on = np.array(on) - 0.015
    # tempo from the bass's onset spacing (walking = quarter notes)
    ioi = np.diff(on)
    ioi = ioi[(ioi > 0.15) & (ioi < 1.2)]
    bpm = 60 / np.median(ioi) if len(ioi) else 160
    act = RNNDownBeatProcessor()(mix_for(track))
    beats = DBNDownBeatTrackingProcessor(beats_per_bar=[4], fps=100, min_bpm=bpm * 0.8, max_bpm=bpm * 1.25)(act)
    bt = beats[:, 0]
    bn = beats[:, 1].astype(int)
    # pitch clarity and level per onset
    y16 = librosa.resample(y, orig_sr=SR, target_sr=16000)
    f0, per = torchcrepe.predict(torch.tensor(y16)[None], 16000, 160, fmin=35, fmax=400, model="tiny",
                                 return_periodicity=True, batch_size=2048,
                                 device="cuda" if torch.cuda.is_available() else "cpu")
    per = per[0].cpu().numpy()
    rms = librosa.feature.rms(y=y, frame_length=1024, hop_length=HOP)[0]
    def level(t):
        i = int(t * SR / HOP)
        return 20 * np.log10(rms[i:i + 6].max() + 1e-9) if i < len(rms) else -99
    def clear(t):
        i = int(t * 100)
        return per[i + 3:i + 10].mean() if i + 10 < len(per) else 0
    dur = len(y) / SR
    C = Counter()
    skip_pos, skip_beat = [], Counter()
    bars = Counter()
    for k in range(len(bt) - 1):
        b0, b1 = bt[k], bt[k + 1]
        if b0 < 10 or b1 > dur - 10:      # skip intros/endings
            continue
        L = b1 - b0
        inside = [t for t in on if b0 - 0.12 * L <= t < b1 - 0.12 * L]
        main = [t for t in inside if t < b0 + 0.2 * L]
        extra = [t for t in inside if t >= b0 + 0.35 * L]
        C["beats"] += 1
        bars[k // 4] += len(inside)
        if not main:
            C["no_main"] += 1
        mlev = level(main[0]) if main else max([level(t) for t in inside] + [-60])
        if extra:
            C["skip"] += 1
            f = (extra[0] - b0) / L
            skip_pos.append(f)
            skip_beat[bn[k]] += 1
            if len(extra) >= 2:
                C["rake"] += 1
        for t in inside:
            C["onsets"] += 1
            if clear(t) < 0.3 or level(t) < mlev - 15:
                C["ghost"] += 1
        # two-beat feel: beat 1 and 3 played, 2 and 4 silent -> counted per bar below
    two = 0
    nbar = 0
    for k in range(0, len(bt) - 4):
        if bn[k] != 1 or bt[k] < 10 or bt[k + 4] > dur - 10:
            continue
        nbar += 1
        hit = []
        for j in range(4):
            b0, b1 = bt[k + j], bt[k + j + 1]
            L = b1 - b0
            hit.append(any(b0 - 0.12 * L <= t < b0 + 0.2 * L for t in on))
        two += hit[0] and hit[2] and not hit[1] and not hit[3]
    return dict(track=track, bpm=60 / np.median(np.diff(bt)), C=C, skip_pos=skip_pos, skip_beat=skip_beat,
                nbar=nbar, two=two, notes_per_bar=4 * C["onsets"] / max(1, C["beats"]))


def line(r):
    C = r["C"]
    sp = r["skip_pos"]
    m = st.median(sp) if sp else None
    sb = r["skip_beat"]
    tot = sum(sb.values()) or 1
    trip = sum(1 for f in sp if 0.6 <= f < 0.75) / max(1, len(sp))
    return ("%-34s %4.0f bpm  beats %4d  notes/bar %.2f  skips %4.1f%% of beats (on beat 1/2/3/4: %s)  skip lands at %s -> %s:1  "
            "triplet-ish %2.0f%%  rakes %.1f%%  ghost/dead %.1f%% of notes  two-beat bars %.1f%%") % (
        r["track"][:34], r["bpm"], C["beats"], r["notes_per_bar"], 100 * C["skip"] / max(1, C["beats"]),
        "/".join("%.0f" % (100 * sb[b] / tot) for b in (1, 2, 3, 4)),
        "%.2f" % m if m else "n/a", "%.2f" % (m / (1 - m)) if m else "n/a", 100 * trip,
        100 * C["rake"] / max(1, C["beats"]), 100 * C["ghost"] / max(1, C["onsets"]), 100 * r["two"] / max(1, r["nbar"]))


rows = []
for track in sorted(os.listdir(ROOT)):
    if mix_for(track):
        r = analyse(track)
        rows.append(r)
        print(line(r), flush=True)
tot = dict(track="ALL (%d tracks)" % len(rows), bpm=st.median(r["bpm"] for r in rows), C=sum((r["C"] for r in rows), Counter()),
           skip_pos=[f for r in rows for f in r["skip_pos"]], skip_beat=sum((r["skip_beat"] for r in rows), Counter()),
           nbar=sum(r["nbar"] for r in rows), two=sum(r["two"] for r in rows))
tot["notes_per_bar"] = 4 * tot["C"]["onsets"] / max(1, tot["C"]["beats"])
print(line(tot))
