"""Per-style drum timing from Magenta's Groove MIDI Dataset (CC BY 4.0).

Drummers played along to a click; the MIDI tempo is that click, so the 16th grid is exact.
usage: python groove_timing.py groove/ > report.txt
"""
import csv, os, sys, statistics as st
from collections import defaultdict
import mido

ROOT = sys.argv[1]
VOICE = {36: "kick", 35: "kick", 38: "snare", 40: "snare rim", 37: "cross-stick",
         42: "hat closed", 22: "hat closed", 44: "hat pedal", 46: "hat open", 26: "hat open",
         51: "ride", 59: "ride", 53: "ride bell"}
GHOST_VEL = 50          # snare head hits softer than this count as ghost notes
POS = ["1", "e", "&", "a"]


def group(style):
    s = style.lower()
    if s.startswith("jazz/swing") or s in ("jazz/mediumfast", "jazz/fast"):
        return ["jazz (swing)", "jazz (all)"]
    top = s.split("/")[0]
    out = []
    if top == "latin":
        out.append("latin (all)")
        if "bossa" in s:
            out.append("latin: bossa")
        if "samba" in s and "reggae" not in s:
            out.append("latin: samba")
    elif top == "jazz":
        out.append("jazz (all)")
        if "funk" in s or "fusion" in s:
            out.append("jazz: funk/fusion")
    elif top in ("funk", "rock", "soul", "hiphop", "neworleans", "afrobeat", "afrocuban", "pop"):
        out.append(top)
    return out


def events(path):
    mf = mido.MidiFile(path)
    tpb = mf.ticks_per_beat
    tempo = 500000
    tick = 0
    out = []
    for msg in mido.merge_tracks(mf.tracks):
        tick += msg.time
        if msg.type == "set_tempo":
            tempo = msg.tempo
        if msg.type == "note_on" and msg.velocity > 0:
            out.append((tick, msg.note, msg.velocity))
    return out, tpb, tempo / 1e6  # seconds per beat (GMD files have one tempo)


stats = defaultdict(lambda: defaultdict(list))   # group -> key -> values
files = defaultdict(int)
for row in csv.DictReader(open(os.path.join(ROOT, "info.csv"))):
    if row["beat_type"] != "beat" or row["time_signature"] != "4-4":
        continue
    gs = group(row["style"])
    if not gs:
        continue
    ev, tpb, spb = events(os.path.join(ROOT, row["midi_filename"]))
    for g in gs:
        files[g] += 1
    for tk, note, vel in ev:
        v = VOICE.get(note)
        if not v:
            continue
        x = tk / tpb                       # position in beats
        beat = int(x)
        frac = x - beat
        k = int(round(frac * 4))           # nearest 16th
        if k == 4:
            k, beat = 0, beat + 1
        off_ms = (frac - k / 4) * spb * 1000 if k or frac < 0.5 else (frac - 1) * spb * 1000
        if v == "snare" and vel < GHOST_VEL:
            v = "ghost snare"
        for g in gs:
            S = stats[g]
            S[(v, k, "off")].append(off_ms)
            S[(v, k, "vel")].append(vel)
            S[(v, "n")].append(1)
            if v in ("snare", "ghost snare"):
                S["snare_all"].append(vel < GHOST_VEL)
            if v in ("hat closed", "hat open", "ride", "hat pedal"):
                if 0.12 < frac < 0.38:
                    S["e_pos"].append(frac)
                if 0.62 < frac < 0.88:
                    S["a_pos"].append(frac)
                if 0.40 < frac < 0.80:
                    S["off8_pos"].append(frac)

ORDER = ["funk", "jazz: funk/fusion", "soul", "rock", "hiphop", "neworleans", "afrobeat", "afrocuban",
         "latin (all)", "latin: bossa", "latin: samba", "jazz (swing)", "jazz (all)", "pop"]
VOICES = ["kick", "snare", "ghost snare", "cross-stick", "snare rim", "hat closed", "hat open", "hat pedal", "ride", "ride bell"]
L = []
P = L.append
P("GROOVE TIMING -- Magenta Groove MIDI Dataset v1.0.0 (10 drummers, ~13.6 h, CC BY 4.0)")
P("=" * 100)
P("Only 'beat' recordings in 4/4 (fills excluded). Drummers played to a click, so the grid is exact.")
P("offset = ms from the nearest 16th grid point (+ = late), shown per 16th position 1 / e / & / a as")
P("mean (sd) [n]; vel = mean MIDI velocity at that position. ghost snare = snare head hit with velocity")
P("< %d. Swing: where hats/ride hits on the 'e' and 'a' land (straight 0.25 / 0.75, triplet-ish" % GHOST_VEL)
P("0.33 / 0.83), and the 16th long:short ratio from the 'e' (x / (0.5 - x)); 8th swing from offbeats")
P("between 0.40 and 0.80 of the beat (ratio x / (1 - x)).")
for g in ORDER:
    if g not in stats:
        continue
    S = stats[g]
    P("")
    P("#" * 100)
    e, a, o8 = S["e_pos"], S["a_pos"], S["off8_pos"]
    em = st.median(e) if e else None
    am = st.median(a) if a else None
    om = st.median(o8) if o8 else None
    P("%s   (%d grooves)   ghost share of snare hits: %.0f%%" % (g.upper(), files[g], 100 * st.mean(S["snare_all"]) if S["snare_all"] else 0))
    P("  swing: 'e' at %s, 'a' at %s -> 16th ratio %s ; offbeat 8th at %s -> 8th ratio %s" % (
        "%.3f" % em if em else "n/a", "%.3f" % am if am else "n/a",
        "%.2f:1" % (em / (0.5 - em)) if em else "n/a", "%.3f" % om if om else "n/a",
        "%.2f:1" % (om / (1 - om)) if om else "n/a"))
    P("  %-12s %7s | %-22s %-22s %-22s %-22s | %s" % ("voice", "hits", "1", "e", "&", "a", "vel 1/e/&/a"))
    for v in VOICES:
        n = len(S[(v, "n")])
        if n < 20:
            continue
        cells, vels = [], []
        for k in range(4):
            xs = S[(v, k, "off")]
            vs = S[(v, k, "vel")]
            cells.append("%+5.1f (%4.1f) [%d]" % (st.mean(xs), st.pstdev(xs), len(xs)) if len(xs) >= 5 else "-")
            vels.append("%.0f" % st.mean(vs) if len(vs) >= 5 else "-")
        P("  %-12s %7d | %-22s %-22s %-22s %-22s | %s" % (v, n, *cells, "/".join(vels)))
print("\n".join(L))
