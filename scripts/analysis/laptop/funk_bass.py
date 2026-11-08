"""Funk / soul / disco bass-line statistics from the Lakh MIDI Dataset (LMD-matched, CC BY 4.0),
selected by Last.fm tags (MSD Last.fm dataset).  Statistics and shapes only.

usage: python funk_bass.py DATA_DIR > report.txt
"""
import json, os, sqlite3, sys, statistics as st, warnings
from collections import Counter, defaultdict
import numpy as np
import pretty_midi

warnings.filterwarnings("ignore")
D = sys.argv[1]
GROUPS = {  # last.fm tag -> group
    "funk": "funk", "p-funk": "funk", "funky": "funk", "jazz funk": "funk", "funk rock": "funk", "funk soul": "funk",
    "acid jazz": "funk", "electro funk": "funk", "go-go": "funk",
    "soul": "soul", "northern soul": "soul", "motown": "soul", "classic soul": "soul", "70s soul": "soul",
    "philly soul": "soul",  # (modern "rnb" and "neo-soul" left out: different bass language)
    "disco": "disco", "nu disco": "disco", "boogie": "disco", "70s disco": "disco",
    "fusion": "fusion", "jazz fusion": "fusion", "jazz-funk": "funk", "smooth jazz": "fusion",
}
MIN_TAG = 25  # last.fm tag weight (0-100)
CANON = ["james brown", "the j.b.'s", "parliament", "funkadelic", "bootsy", "sly & the family stone",
         "sly and the family stone", "tower of power", "the meters", "kool & the gang", "kool and the gang",
         "earth, wind & fire", "earth wind & fire", "ohio players", "commodores", "rick james", "prince",
         "chic", "brothers johnson", "the brothers johnson", "cameo", "zapp", "gap band", "the gap band",
         "average white band", "jamiroquai", "stevie wonder", "wild cherry", "herbie hancock",
         "isley brothers", "the isley brothers", "bar-kays", "con funk shun", "lakeside", "graham central station",
         "rufus", "heatwave", "george duke", "marcus miller", "level 42", "the crusaders", "war",
         "kc & the sunshine band", "kc and the sunshine band", "maceo parker", "fred wesley", "the blackbyrds",
         "booker t", "the temptations", "marvin gaye", "michael jackson", "the jacksons", "jackson 5",
         "incognito", "brand new heavies", "the brand new heavies", "average white band", "chaka khan",
         "shalamar", "lipps inc", "a taste of honey", "sister sledge", "the time", "mtume", "slave",
         "dazz band", "one way", "b.t. express", "fatback", "the fatback band", "skyy", "brass construction"]

DEGNAME = ["1", "b2", "2", "b3", "3", "4", "b5", "5", "b6", "6", "b7", "7"]


def select():
    db = sqlite3.connect(os.path.join(D, "lastfm_tags.db"))
    tags = {r[0]: r[1] for r in db.execute("select ROWID, tag from tags")}
    want = {rid: GROUPS[t.lower()] for rid, t in tags.items() if t.lower() in GROUPS}
    best = {}
    q = "select tids.tid, tid_tag.tag, tid_tag.val from tid_tag join tids on tids.ROWID=tid_tag.tid where tid_tag.tag in (%s)" % \
        ",".join(map(str, want))
    for tid, tag, val in db.execute(q):
        if val >= MIN_TAG:
            g = want[tag]
            if tid not in best or val > best[tid][1]:
                best[tid] = (g, val)
    return {tid: g for tid, (g, _) in best.items()}


def names():
    out = {}
    for line in open(os.path.join(D, "unique_tracks.txt"), encoding="latin-1"):
        p = line.rstrip("\n").split("<SEP>")
        if len(p) == 4:
            out[p[0]] = (p[2], p[3])
    return out


def bass_instrument(pm):
    cands = [i for i in pm.instruments if not i.is_drum and 32 <= i.program <= 39 and len(i.notes) > 30]
    if not cands:
        low = [i for i in pm.instruments if not i.is_drum and len(i.notes) > 30 and
               np.median([n.pitch for n in i.notes]) < 50]
        cands = low
    return max(cands, key=lambda i: len(i.notes)) if cands else None


def analyse(path):
    pm = pretty_midi.PrettyMIDI(path)
    ts = pm.time_signature_changes
    if ts and any((t.numerator, t.denominator) != (4, 4) for t in ts):
        return None
    bass = bass_instrument(pm)
    if bass is None:
        return None
    beats = pm.get_beats()
    if len(beats) < 33:
        return None
    # 16th grid by subdividing each beat
    grid = np.concatenate([np.linspace(b0, b1, 4, endpoint=False) for b0, b1 in zip(beats[:-1], beats[1:])])
    downs = set(np.searchsorted(grid, pm.get_downbeats()[:], side="left"))
    def q(t):
        i = int(np.argmin(np.abs(grid - t)))
        return i, t - grid[i]
    kicks = set()
    for inst in pm.instruments:
        if inst.is_drum:
            for n in inst.notes:
                if n.pitch in (35, 36):
                    kicks.add(q(n.start)[0])
    notes = []
    errs = []
    for n in sorted(bass.notes, key=lambda n: (n.start, n.pitch)):
        i, e = q(n.start)
        if notes and notes[-1][0] == i:
            continue  # one note per 16th: the lowest (double stops and octaves collapse to it)
        sixteenth = (grid[min(i + 1, len(grid) - 1)] - grid[i]) or 0.1
        errs.append(abs(e) / sixteenth)
        notes.append((i, n.pitch, (n.end - n.start) / sixteenth, n.velocity))
    if len(notes) < 32 or np.median(errs) > 0.2:
        return None
    # bars: assume the first downbeat in grid indices and 16 sixteenths per bar
    d0 = min(downs) if downs else 0
    bars = defaultdict(list)
    for i, p, dur, v in notes:
        if i >= d0:
            bars[(i - d0) // 16].append(((i - d0) % 16, p, dur, v))
    return bars, kicks, d0, (len(kicks) > 0)


def bar_root(bn):
    on1 = [p for pos, p, d, v in bn if pos == 0]
    if on1:
        return on1[0]
    pcs = Counter()
    for pos, p, d, v in bn:
        pcs[p % 12] += d
    pc = pcs.most_common(1)[0][0]
    return min(p for pos, p, d, v in bn if p % 12 == pc)


def norm_title(t):
    import re
    t = re.sub(r"[\(\[].*?[\)\]]", "", t.lower())
    t = re.sub(r"\b(lp|single|album|radio|extended|remaster(ed)?|version|edit|mix|mono|stereo|live)\b", "", t)
    return re.sub(r"[^a-z0-9]+", " ", t).strip()


def main():
    sel = select()
    nm = names()
    # the classic-funk canon by artist name, whatever the tags say
    for tid, (artist, title) in nm.items():
        a = artist.lower()
        if any(a == c or a.startswith(c + " ") or a.startswith(c + ",") for c in CANON):
            sel[tid + "#canon"] = "canon"
    scores = json.load(open(os.path.join(D, "match_scores.json")))
    root = os.path.join(D, "lmd_matched")
    S = defaultdict(Counter)
    R = defaultdict(list)
    riffs = defaultdict(Counter)
    riff_src = {}
    rhythm_only = defaultdict(Counter)
    songs = Counter()
    used = defaultdict(list)
    seen = defaultdict(set)
    seen_title = set()
    song_riffs = defaultdict(lambda: defaultdict(Counter))
    rhythm_songs = defaultdict(lambda: defaultdict(set))
    song_shapes = defaultdict(lambda: defaultdict(set))
    for tidg, g in sel.items():
        tid = tidg.split("#")[0]
        if tid not in scores:
            continue
        md5 = max(scores[tid], key=scores[tid].get)
        path = os.path.join(root, tid[2], tid[3], tid[4], tid, md5 + ".mid")
        if not os.path.exists(path):
            continue
        try:
            res = analyse(path)
        except Exception:
            continue
        if not res:
            continue
        bars, kicks, d0, has_drums = res
        artist, title = nm.get(tid, ("?", "?"))
        key_song = (artist.lower(), norm_title(title))
        if key_song in seen[g]:
            continue
        seen[g].add(key_song)
        songs[g] += 1
        used[g].append("%s - %s" % (artist, title))
        C = S[g]
        keys = sorted(bars)
        vels = [v for k in keys for _, _, _, v in bars[k]]
        vmed = st.median(vels) if vels else 64
        for k in keys:
            bn = bars[k]
            if len(bn) < 2:
                continue
            r = bar_root(bn)
            pat = ["."] * 16
            for pos, p, dur, v in bn:
                pat[pos] = "x"
                deg = (p - r) % 12
                C["deg_" + DEGNAME[deg]] += 1
                C["notes"] += 1
                if p - r >= 12 and deg in (0, 7):
                    C["pop"] += 1
                    C["pop_pos_%d" % pos] += 1
                if p - r >= 12:
                    C["above8va"] += 1
                if p < r:
                    C["below_root"] += 1
                if dur < 0.6:
                    C["dead"] += 1
                elif dur < 1.2:
                    C["short"] += 1
                elif dur < 3.5:
                    C["mid"] += 1
                else:
                    C["held"] += 1
                if v < 0.6 * vmed:
                    C["ghost"] += 1
                C["pos_%d" % pos] += 1
            C["bars"] += 1
            R[g].append("".join(pat))
            # the ONE and the 'a' of 4
            one = [x for x in bn if x[0] == 0]
            if one:
                C["one"] += 1
                C["one_len_sum"] += min(one[0][2], 16)
                C["one_root"] += (one[0][1] - r) % 12 == 0
            a4 = [x for x in bn if x[0] == 15]
            if a4:
                C["a4"] += 1
                nxt = bars.get(k + 1)
                if nxt:
                    r2 = bar_root(nxt)
                    iv = r2 - a4[0][1]
                    C["a4_next"] += 1
                    C["a4_half_below"] += iv == 1
                    C["a4_half_above"] += iv == -1
                    C["a4_same"] += iv == 0
                    C["a4_tie_into_one"] += not any(x[0] == 0 for x in nxt)
            # approaches: chromatic step into the next bar's downbeat note
            nxt = bars.get(k + 1)
            if nxt and bn:
                last = max(bn)
                n1 = [x for x in nxt if x[0] == 0]
                if n1 and last[0] >= 12:
                    C["into1"] += 1
                    C["into1_chrom"] += abs(n1[0][1] - last[1]) == 1
            # kick lock
            if kicks:
                kk = {(i - d0) % 16 for i in kicks if (i - d0) // 16 == k}
                bb = {x[0] for x in bn}
                C["kick_bars"] += 1
                C["kick_hits"] += len(kk)
                C["bass_hits_k"] += len(bb)
                C["both"] += len(kk & bb)
        # 2-bar riffs, every bar pair starting on an even bar
        for k in keys:
            if k % 2 or k + 1 not in bars or len(bars[k]) < 2:
                continue
            a, b = bars[k], bars[k + 1]
            r = bar_root(a)
            pa = "".join("x" if any(x[0] == i for x in a) else "." for i in range(16))
            pb = "".join("x" if any(x[0] == i for x in b) else "." for i in range(16))
            C["pairs"] += 1
            C["pair_same_rhythm"] += pa == pb
            seq_a = [(x[1] - r) for x in sorted(a)]
            seq_b = [(x[1] - r) for x in sorted(b)]
            C["pair_same_all"] += pa == pb and seq_a == seq_b
            C["pair_same_first_half"] += pa[:8] == pb[:8]
            degs = " ".join(deg_label(x[1] - r) for x in sorted(a)) + " | " + " ".join(deg_label(x[1] - r) for x in sorted(b))
            key = (pa + " " + pb, degs)
            riffs[g][key] += 1
            riff_src.setdefault((g, key), set()).add("%s - %s" % (artist, title))
            song_riffs[g][(artist, title)][key] += 1
            rhythm_songs[g][pa].add(norm_title(title))
            rhythm_songs[g][pb].add(norm_title(title))
            coarse = " ".join(cls(x[1] - r) for x in sorted(a)) + " | " + " ".join(cls(x[1] - r) for x in sorted(b))
            song_shapes[g][(pa + " " + pb, coarse)].add("%s - %s" % (artist, title))
            song_shapes[g + "/1bar"][(pa, " ".join(cls(x[1] - r) for x in sorted(a)))].add("%s - %s" % (artist, title))
            rhythm_only[g][pa + " " + pb] += 1
    report(S, R, riffs, riff_src, rhythm_only, songs, used, song_shapes, song_riffs, rhythm_songs)


def cls(semis):
    """Degree with the octave folded, except notes an octave or more above the root (pops: ^)."""
    return ("^" if semis >= 12 else "") + DEGNAME[semis % 12]


def deg_label(semis):
    o = semis // 12
    d = DEGNAME[semis % 12]
    return d + ("'" * o if o > 0 else "," * (-o) if o < 0 else "")


def report(S, R, riffs, riff_src, rhythm_only, songs, used, song_shapes, song_riffs, rhythm_songs):
    L = []
    P = L.append
    P("FUNK BASS LINES -- Lakh MIDI Dataset (LMD-matched, CC BY 4.0), tracks picked by Last.fm tags")
    P("=" * 100)
    P("Statistics and shapes only; song names are given as study material. These are fan-made MIDI files")
    P("(of varying accuracy), not transcriptions of the records, so read shapes as 'how people hear and")
    P("re-play these grooves'. Bass = GM bass program (32-39) or else the lowest melodic track; 4/4 only;")
    P("onsets snapped to the 16th grid (files with median snap error > 20% of a 16th dropped). Root of a bar")
    P("= the bass note on beat 1, else the commonest pitch class. Degrees: 1 5 b7 etc. relative to that root;")
    P("' = an octave up, , = an octave down. Pattern strings: x = onset on that 16th (1 e & a 2 e & a ...).")
    P("")
    for g in ("canon", "funk", "disco", "soul", "fusion"):
        C = S[g]
        if not C["bars"]:
            continue
        n = C["notes"]
        P("#" * 100)
        P("%s: %d songs, %d bars, %d notes" % (g.upper(), songs[g], C["bars"], n))
        P("#" * 100)
        P("onsets per bar: %.2f   share of bars with a note on the ONE: %.0f%%" % (n / C["bars"], 100 * C["one"] / C["bars"]))
        P("onset share by 16th position (1 e & a | 2 e & a | 3 e & a | 4 e & a):")
        P("   " + " ".join("%4.0f%%" % (100 * C["pos_%d" % i] / C["bars"]) + (" |" if i % 4 == 3 and i < 15 else "")
                           for i in range(16)))
        P("degree vocabulary (share of notes): " + "  ".join(
            "%s %.1f%%" % (d, 100 * C["deg_" + d] / n) for d in sorted(DEGNAME, key=lambda d: -C["deg_" + d])))
        P("octave pops (root or 5th an octave+ above the bar root): %.1f%% of notes; positions: %s" % (
            100 * C["pop"] / n, " ".join("%s:%d" % (posname(i), C["pop_pos_%d" % i]) for i in
                                         sorted(range(16), key=lambda i: -C["pop_pos_%d" % i])[:6])))
        P("notes an octave or more above the root: %.1f%%; below the bar root: %.1f%%" % (
            100 * C["above8va"] / n, 100 * C["below_root"] / n))
        P("lengths (in 16ths): dead/clipped < 0.6: %.0f%%   short 0.6-1.2: %.0f%%   1.2-3.5: %.0f%%   held >= 3.5: %.0f%%" % (
            100 * C["dead"] / n, 100 * C["short"] / n, 100 * C["mid"] / n, 100 * C["held"] / n))
        P("ghost notes (velocity < 60%% of the track median): %.1f%%" % (100 * C["ghost"] / n))
        P("the ONE: %.0f%% of bars; average length %.2f 16ths; on the root %.0f%% of the time" % (
            100 * C["one"] / C["bars"], C["one_len_sum"] / max(1, C["one"]), 100 * C["one_root"] / max(1, C["one"])))
        P("'a' of 4 (the last 16th): %.0f%% of bars; when the next bar follows: a half step below its root %.0f%%," % (
            100 * C["a4"] / C["bars"], 100 * C["a4_half_below"] / max(1, C["a4_next"])) +
          " above %.0f%%, the same note %.0f%%; tied over (no new ONE) %.0f%%" % (
              100 * C["a4_half_above"] / max(1, C["a4_next"]), 100 * C["a4_same"] / max(1, C["a4_next"]),
              100 * C["a4_tie_into_one"] / max(1, C["a4_next"])))
        P("last note of a bar (on beat 4) -> next bar's ONE by a half step: %.0f%%" % (
            100 * C["into1_chrom"] / max(1, C["into1"])))
        P("2-bar riffs: bar 2 repeats bar 1's rhythm %.0f%%, rhythm and notes %.0f%%, the first half only %.0f%%" % (
            100 * C["pair_same_rhythm"] / max(1, C["pairs"]), 100 * C["pair_same_all"] / max(1, C["pairs"]),
            100 * C["pair_same_first_half"] / max(1, C["pairs"])))
        if C["kick_bars"]:
            P("bass/kick lock (bars with a drum track): %.0f%% of kick hits have a bass onset on the same 16th;" % (
                100 * C["both"] / max(1, C["kick_hits"])) + " %.0f%% of bass onsets land on a kick" % (
                100 * C["both"] / max(1, C["bass_hits_k"])))
        top = Counter(R[g]).most_common(15)
        P("top 1-bar rhythms: " + "  ".join("%s %.1f%%" % (p, 100 * c / C["bars"]) for p, c in top))
        P("")
        P("40 MAIN GROOVES (%s): each song's most repeated 2-bar riff, one per song (titles merged), only grooves" % g)
        P("that repeat >= 4 times and are not plain repeated roots; at most 2 per artist; ranked by repeats.")
        P("Rhythm bar1 bar2 | degrees from the bar-1 root (' = octave up, , = octave down). Study material only:")
        P("these are shapes to rhyme with, not lines to copy.")
        cand = []
        for (artist, title), cnt in song_riffs[g].items():
            (rh, dg), n = cnt.most_common(1)[0]
            flat = dg.replace("|", "").split()
            if n < 4 or len(flat) < 5 or set(x.strip("',") for x in flat) <= {"1"} and rh.count("x") <= 8:
                continue
            cand.append((n, artist, title, rh, dg))
        cand.sort(key=lambda c: -c[0])
        per_artist, titles, k = Counter(), set(), 0
        for n, artist, title, rh, dg in cand:
            t = norm_title(title)
            if per_artist[artist.lower()] >= 2 or t in titles:
                continue
            per_artist[artist.lower()] += 1
            titles.add(t)
            k += 1
            P("%2d. %s   %-46s x%-3d %s - %s" % (k, rh, dg, n, artist, title))
            if k == 40:
                break
        P("")
        P("TOP 30 ONE-BAR RHYTHMS (%s) by the number of different songs that use them" % g)
        for rh, ss in sorted(rhythm_songs[g].items(), key=lambda kv: -len(kv[1]))[:30]:
            P("    %s   %3d songs (%.0f%%)" % (rh, len(ss), 100 * len(ss) / max(1, songs[g])))
        P("")
        P("TOP 25 ONE-BAR SHAPES (%s), by number of songs" % g)
        rows = sorted(song_shapes[g + "/1bar"].items(), key=lambda kv: -len(kv[1]))
        k = 0
        for (rh, dg), src in rows:
            if set(dg.split()) <= {"1"} and rh in ("x.x." * 4, "x..." * 4):
                continue
            k += 1
            P("%2d. %s   %-28s %d songs" % (k, rh, dg, len(src)))
            if k == 25:
                break
        P("")
        P("TOP 15 TWO-BAR RHYTHMS (%s, any notes)" % g)
        for p, c in rhythm_only[g].most_common(15):
            P("    %s   %.1f%% of 2-bar windows" % (p, 100 * c / max(1, C["pairs"])))
        P("")
    P("SONGS USED")
    for g in ("canon", "funk", "disco", "soul", "fusion"):
        P("%s (%d): %s" % (g, len(used[g]), "; ".join(sorted(used[g]))))
    print("\n".join(L))


def posname(i):
    return "%d%s" % (i // 4 + 1, ["", "e", "&", "a"][i % 4])


if __name__ == "__main__":
    main()
