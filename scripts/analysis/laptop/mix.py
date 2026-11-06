"""Rhythm-section level under the soloist, from Demucs htdemucs_6s stems.

usage: mix.py <report.txt> horn:<stems_root> [trio:<stems_root> ...]
stems_root/<track>/{drums,bass,piano,guitar,vocals,other}.wav

horn: soloist = other + vocals (Demucs puts the tenor in 'other' but often a muted trumpet
      in 'vocals'), comp = piano + guitar (there is no guitar on these records; the guitar
      stem is piano/horn bleed), band = drums + bass + comp.
trio: soloist = piano + guitar, band = drums + bass (piano trio: the pianist is the soloist).
"""
import sys, os, json
import numpy as np
import soundfile as sf

N = 2048
GATE = -40.0

# WJazzD solos on these tracks (transcription_info.solotime), used to label sections
# (this RVG remaster runs 24.6 s later than the audio WJazzD used: found by matching the DB's notes)
DB_SOLOS = {"04 - Oleo": [("Miles Davis", 54.6, 111.6), ("John Coltrane", 112.6, 226.6), ("Miles Davis", 285.6, 341.6)]}


def a_weight(f):
    def ra(f):
        f2 = f ** 2
        return (12194 ** 2 * f2 ** 2) / ((f2 + 20.6 ** 2) * np.sqrt((f2 + 107.7 ** 2) * (f2 + 737.9 ** 2)) * (f2 + 12194 ** 2))
    return ra(f) / ra(np.array(1000.0))


def levels(x, sr):
    """Per 2048-sample window: A-weighted level and flat RMS level, dBFS, mono."""
    x = x.mean(axis=1) if x.ndim > 1 else x
    nwin = len(x) // N
    fr = x[:nwin * N].reshape(nwin, N)
    win = np.hanning(N)
    spec = np.abs(np.fft.rfft(fr * win, axis=1)) ** 2
    w = a_weight(np.fft.rfftfreq(N, 1 / sr)) ** 2
    norm = (win ** 2).sum() * N / 2
    return 10 * np.log10((spec * w).sum(axis=1) / norm + 1e-12), 10 * np.log10((fr ** 2).mean(axis=1) + 1e-12)


def sections(solo_flat, solo_a, comp_a, hop, trio):
    """Soloist-active sections: 10 s smoothed soloist level above gate+10 dB and (horn mode) not more
    than 6 dB under the comping, so piano solos with horn bleed stay out. Sections >= 15 s."""
    if trio:
        return [(0, len(solo_flat) * hop)]
    k = int(10 / hop)
    sm = lambda v: 10 * np.log10(np.convolve(10 ** (v / 10), np.ones(k) / k, mode="same") + 1e-12)
    act = (sm(solo_flat) > GATE + 10) & (sm(solo_a) > sm(comp_a) - 6)
    out, start = [], None
    for i, a in enumerate(np.append(act, False)):
        if a and start is None:
            start = i
        if not a and start is not None:
            if (i - start) * hop >= 15:
                out.append((start * hop, i * hop))
            start = None
    return out


def label(track, a, b):
    best = None
    for who, s, e in DB_SOLOS.get(track, []):
        ov = min(b, e) - max(a, s)
        if ov > 0.5 * (b - a) and (best is None or ov > best[1]):
            best = (who, ov)
    return best[0] + " (WJazzD solo)" if best else ""


def run(root, trio, root4):
    rows = []
    for track in sorted(os.listdir(root)):
        d = os.path.join(root, track)
        d4 = os.path.join(root4, track)
        st = {}
        for s in ("drums", "bass", "piano", "guitar", "vocals", "other"):
            st[s], sr = sf.read(os.path.join(d, s + ".wav"))
        st4 = {}
        for s in ("drums", "bass", "other", "vocals"):
            st4[s], _ = sf.read(os.path.join(d4, s + ".wav"))
        n = min(len(st["other"]), len(st4["bass"]))
        st = {k: v[:n] for k, v in st.items()}
        st4 = {k: v[:n] for k, v in st4.items()}
        if trio:
            # piano trio: the 4-stem 'other' is the piano (nothing else is there)
            sig = dict(solo=st4["other"] + st4["vocals"], bass=st4["bass"], drums=st4["drums"],
                       band=st4["bass"] + st4["drums"])
        else:
            # horn soloist from the 6-stem model; bass and drums from the 4-stem model (the 6-stem model files
            # Chambers' walking bass under 'guitar' for long stretches); comp = 6-stem piano only
            # comp = 6-stem piano + the 6-stem 'guitar' above 300 Hz (that stem holds piano comping AND misfiled
            # walking bass; the bass mostly sits below 300 Hz)
            import scipy.signal as ss
            bb, aa = ss.butter(4, 300 / (sr / 2), btype="high")
            comp = st["piano"] + ss.filtfilt(bb, aa, st["guitar"], axis=0)
            sig = dict(solo=st["other"] + st["vocals"], comp=comp, bass=st4["bass"], drums=st4["drums"],
                       band=st4["drums"] + st4["bass"] + comp,
                       comp_check=st4["other"] + st4["vocals"] - st["other"] - st["vocals"])
        lv = {k: levels(v, sr) for k, v in sig.items()}
        hop = N / sr
        comp_a = lv["comp"][0] if "comp" in lv else lv["band"][0]
        for a, b in sections(lv["solo"][1], lv["solo"][0], comp_a, hop, trio):
            i0, i1 = int(a / hop), min(int(b / hop), len(lv["solo"][1]))
            sel = np.arange(i0, i1)
            sel = sel[lv["solo"][1][sel] > GATE]
            if len(sel) < 50:
                continue
            r = dict(kind="trio" if trio else "horn", track=track, n=int(len(sel)),
                     span="%d:%02d-%d:%02d" % (a // 60, a % 60, b // 60, b % 60), who=label(track, a, b))
            for k in ("comp", "comp_check", "bass", "drums", "band"):
                if k in lv:
                    r[k] = float(np.median(lv["solo"][0][sel] - lv[k][0][sel]))
                    r[k + "_flat"] = float(np.median(lv["solo"][1][sel] - lv[k][1][sel]))
            r["louder"] = float(100 * np.mean(lv["band"][0][sel] > lv["solo"][0][sel]))
            r["louder_flat"] = float(100 * np.mean(lv["band"][1][sel] > lv["solo"][1][sel]))
            rows.append(r)
    return rows


def f(r, k):
    return "%+6.1f" % r[k] if k in r else "   n/a"


def main(report, *srcs):
    rows = []
    for s in srcs:
        kind, root, root4 = s.split(":", 2)
        rows += run(root, kind == "trio", root4)
    out = []
    P = out.append
    P("MIX: how far the rhythm section sits under the soloist (Demucs htdemucs_6s stems)")
    P("=" * 96)
    P("Per %d-sample window (44.1 kHz) where the soloist stem is above %d dBFS. Values = median over windows" % (N, GATE))
    P("of (soloist level - stem level) in dB, so +10 = the stem sits 10 dB UNDER the soloist. A = A-weighted")
    P("(as asked); flat = unweighted RMS, shown because A-weighting takes 20-30 dB off a walking bass.")
    P("'louder' = % of windows where the band (sum of its stems) is louder than the soloist.")
    P("HORN tracks (Miles Davis Quintet, Relaxin', 1956, RVG remaster): soloist = htdemucs_6s 'other' + 'vocals'")
    P("(Coltrane's tenor lands in 'other', Miles' Harmon-muted trumpet often in 'vocals'); comp = htdemucs_6s")
    P("'piano' + its 'guitar' stem above 300 Hz (piano comping lands in both); bass and drums from the 4-stem")
    P("htdemucs model, because htdemucs_6s files Paul Chambers'")
    P("walking bass under 'guitar' for long stretches (there is no guitar on the date); band = drums+bass+comp.")
    P("Sections = where the soloist is steadily active (10 s smoothing, >= 15 s), so heads and trades with a")
    P("horn lead are included; Oleo sections are labelled where they overlap WJazzD's solo times.")
    P("TRIO tracks (Red Garland's Piano, 1957): soloist = 4-stem 'other' (only the piano is there), band =")
    P("4-stem bass+drums, whole")
    P("tracks (the pianist leads throughout). Separation bleed makes all of this rough, about +-2-3 dB.")
    P("")
    hdr = "%-5s %-26s %-11s %5s | %6s %6s %6s %6s %6s | %6s %6s %6s | %6s" % (
        "kind", "track", "section", "wins", "comp", "bass", "drums", "band", "louder", "bass", "band", "louder", "who")
    P("%-51s | %-34s | %-20s |" % ("", "A-weighted (dB under / %)", "flat"))
    P(hdr)
    for r in rows:
        P("%-5s %-26s %-11s %5d | %s %s %s %s %5.1f%% | %s %s %5.1f%% | %s" % (
            r["kind"], r["track"][:26], r["span"], r["n"], f(r, "comp"), f(r, "bass"), f(r, "drums"), f(r, "band"),
            r["louder"], f(r, "bass_flat"), f(r, "band_flat"), r["louder_flat"], r["who"]))
    P("")
    P("MEDIANS PER STYLE (over sections)")
    for kind, lab in (("horn", "1956 hard-bop quintet, horn soloist"), ("trio", "1957 piano trio, piano soloist")):
        sub = [r for r in rows if r["kind"] == kind]
        if not sub:
            continue
        med = lambda k: float(np.median([r[k] for r in sub if k in r])) if any(k in r for r in sub) else float("nan")
        P("  %s (%d sections, %d tracks):" % (lab, len(sub), len({r["track"] for r in sub})))
        P("    A-weighted: comp %+.1f  bass %+.1f  drums %+.1f  band %+.1f dB under; band louder in %.1f%% of windows" % (
            med("comp"), med("bass"), med("drums"), med("band"), med("louder")))
        if any("comp_check" in r for r in sub):
            P("    comp cross-check (4-stem 'other' minus the horn stems): %+.1f dB under. The two comp estimates" % med("comp_check"))
            P("    bracket the truth: the subtraction leaves residue (reads louder), the piano+guitar>300 Hz")
            P("    estimate misses comping that Demucs filed elsewhere (reads quieter).")
        P("    flat:       comp %+.1f  bass %+.1f  drums %+.1f  band %+.1f dB under; band louder in %.1f%% of windows" % (
            med("comp_flat"), med("bass_flat"), med("drums_flat"), med("band_flat"), med("louder_flat")))
    P("  Separation-model sensitivity: an earlier run with all stems from htdemucs_6s gave quintet drums 14.4,")
    P("  bass 27.7 (A) and comp 15.3 dB under; per-stem numbers move by several dB with the separation, the")
    P("  band total (7.4 vs 6.8) and the share of windows where the band is louder (35 vs 37%) barely move.")
    P("  Engine today (for comparison): band 4.4-7.8 dB under, louder in 4-21% of windows;")
    P("    comp 11-13.6, bass 8-11.7, drums 9-12.9 dB under.")
    open(report, "w").write("\n".join(out) + "\n")
    json.dump(rows, open(report + ".json", "w"), indent=1)
    print("\n".join(out))


if __name__ == "__main__":
    main(*sys.argv[1:])
