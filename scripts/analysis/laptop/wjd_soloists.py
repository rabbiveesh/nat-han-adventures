import sqlite3, sys, re, statistics as st
from collections import Counter, defaultdict

DB = sys.argv[1]
c = sqlite3.connect(DB)

# ---------- solos ----------
solos = {}
for melid, perf, inst, style, tempo, rd in c.execute(
        "select s.melid, s.performer, s.instrument, s.style, s.avgtempo, t.recordingdate "
        "from solo_info s left join track_info t using(trackid)"):
    m = re.search(r'19\d\d', rd or '')
    solos[melid] = dict(perf=perf, inst=inst, style=style, tempo=tempo, year=int(m.group()) if m else None)

notes = defaultdict(list)
for row in c.execute("select melid,onset,pitch,duration,division,bar,beat,tatum,num,beatdur from melody order by melid,eventid"):
    melid, on, p, dur, div, bar, beat, tat, num, bd = row
    pos = bar * num + (beat - 1) + (tat - 1) / div
    notes[melid].append(dict(on=on, p=int(p), dur=dur, div=div, bar=bar, beat=beat, tat=tat, num=num,
                             bd=bd, pos=pos, durb=dur / bd if bd else 0.5))

sections = defaultdict(lambda: defaultdict(list))
for melid, typ, s, e, v in c.execute("select melid,type,start,end,value from sections order by melid,start"):
    sections[melid][typ].append((s, e, v))

# ---------- chord parsing ----------
PC = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}
def parse_chord(sym):
    if not sym or sym == 'NC':
        return None
    sym = sym.split('/')[0]
    m = re.match(r'([A-G])([#b]?)(.*)', sym)
    if not m:
        return None
    root = (PC[m.group(1)] + {'': 0, '#': 1, 'b': -1}[m.group(2)]) % 12
    q = m.group(3)
    if q.startswith('m7b5'):
        kind = 'hdim'
    elif q.startswith('o'):
        kind = 'dim7' if q.startswith('o7') else 'dim'
    elif q.startswith('-'):
        kind = 'min7' if q[1:].startswith('7') or q[1:].startswith('79') else ('minmaj' if q[1:].startswith('j') else ('min6' if q[1:].startswith('6') else 'min'))
    elif q.startswith('+'):
        kind = 'aug'
    elif q.startswith('sus'):
        kind = 'sus'
    elif q.startswith('j'):
        kind = 'maj7'
    elif q.startswith('6'):
        kind = 'maj6'
    elif q.startswith('7'):
        kind = 'alt' if 'alt' in q else 'dom'
    else:
        kind = 'maj'
    return root, kind

def classify(iv, kind):
    """interval above root -> chordal-diatonic class"""
    minor3 = kind in ('min', 'min7', 'min6', 'minmaj', 'hdim', 'dim', 'dim7')
    if iv == 0: return 'root'
    if iv == 3 and minor3: return '3'
    if iv == 4 and not minor3 and kind != 'sus': return '3'
    if kind in ('hdim', 'dim', 'dim7'):
        if iv == 6: return '5'
    elif kind == 'aug':
        if iv == 8: return '5'
    elif iv == 7:
        return '5'
    if kind == 'alt' and iv == 7: return '5'
    sev = {'dom': {10}, 'alt': {10}, 'sus': {10}, 'min7': {10}, 'hdim': {10}, 'maj7': {11}, 'minmaj': {11},
           'dim7': {9}, 'maj': {11}, 'min': {10, 11}, 'aug': {10, 11}, 'maj6': set(), 'min6': set(), 'dim': {9}}[kind]
    if iv in sev: return '7'
    if iv == 9 and kind in ('maj6', 'min6'): return '6/13'
    if iv == 1 and kind not in ('maj7', 'maj', 'maj6', 'minmaj'): return 'b9'
    if iv == 2: return '9'
    if iv == 3 and not minor3 and kind != 'sus': return '#9'
    if iv == 5: return '11'
    if iv == 6 and kind not in ('hdim', 'dim', 'dim7'): return '#11'
    if iv == 8 and kind not in ('aug', 'maj6', 'maj7', 'maj'): return 'b13'
    if iv == 9: return '6/13'
    return 'other'

CLASSES = ['root', '3', '5', '7', '9', '11', '6/13', 'b9', '#9', '#11', 'b13', 'other']
CHORD_TONES = {'root', '3', '5', '7'}

def ioi_class(x):
    for v, name in ((0.25, '16th'), (1 / 3, '8th-trip'), (0.5, '8th'), (2 / 3, 'qtr-trip'), (0.75, 'dot-8th'), (1.0, 'quarter')):
        if abs(x - v) < 0.02: return name
    if x < 0.23: return 'faster'
    if x > 1.02: return 'longer'
    return 'other'
RCLASSES = ['faster', '16th', '8th-trip', '8th', 'qtr-trip', 'dot-8th', 'quarter', 'longer', 'other']

PENTA = [frozenset((r + x) % 12 for x in (0, 2, 4, 7, 9)) for r in range(12)]
DIATONIC = [frozenset((r + x) % 12 for x in (0, 2, 4, 5, 7, 9, 11)) for r in range(12)]

# ---------- per-solo annotation ----------
def annotate(melid):
    ns = notes[melid]
    sec = sections[melid]
    for i, n in enumerate(ns):
        n['phrase'] = None; n['cls'] = None; n['pfinal'] = False; n['pfirst'] = False
    for k, (s, e, v) in enumerate(sec['PHRASE']):
        for i in range(s, min(e, len(ns) - 1) + 1):
            ns[i]['phrase'] = k
        if s < len(ns): ns[s]['pfirst'] = True
        if e < len(ns): ns[e]['pfinal'] = True
    for s, e, v in sec['CHORD']:
        ch = parse_chord(v)
        if ch is None: continue
        root, kind = ch
        for i in range(s, min(e, len(ns) - 1) + 1):
            ns[i]['cls'] = classify((ns[i]['p'] - root) % 12, kind)
for m in notes: annotate(m)

# ---------- groups ----------
def yr(m): return solos[m]['year'] or 0
G = {}
def P(name): return [m for m in solos if solos[m]['perf'] == name]
G['Miles Davis'] = P('Miles Davis')
G['Miles 1950s'] = [m for m in G['Miles Davis'] if yr(m) < 1960]
G['Miles 1960s'] = [m for m in G['Miles Davis'] if yr(m) >= 1960]
G['John Coltrane'] = P('John Coltrane')
G['Coltrane <1960'] = [m for m in G['John Coltrane'] if yr(m) < 1960]
G['Coltrane 1960+'] = [m for m in G['John Coltrane'] if yr(m) >= 1960]
for p in ['Charlie Parker', 'Sonny Rollins', 'Wayne Shorter', 'Herbie Hancock']: G[p] = P(p)
G['ALL'] = list(solos)
for s in ['TRADITIONAL', 'SWING', 'BEBOP', 'COOL', 'HARDBOP', 'POSTBOP']:
    G['style ' + s] = [m for m in solos if solos[m]['style'] == s]

def q(xs, f):
    xs = sorted(xs)
    if not xs: return float('nan')
    k = (len(xs) - 1) * f; lo = int(k); hi = min(lo + 1, len(xs) - 1)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)
def miqr(xs): return f"{q(xs,.5):.2f} [{q(xs,.25):.2f}-{q(xs,.75):.2f}]"
def pct(a, b): return f"{100*a/b:.1f}" if b else 'nan'

def phrases_of(m):
    ns = notes[m]
    out = []
    for s, e, v in sections[m]['PHRASE']:
        e = min(e, len(ns) - 1)
        if s <= e: out.append(ns[s:e + 1])
    return out

def intervals_in_phrases(m):
    for ph in phrases_of(m):
        yield ph, [ph[i + 1]['p'] - ph[i]['p'] for i in range(len(ph) - 1)]

def stats(ms):
    R = {}
    nn = sum(len(notes[m]) for m in ms)
    R['n'] = (len(ms), nn)
    # a. density
    bins = defaultdict(lambda: [0, 0, 0.0, 0])  # notes, bars, secs, solos
    rest_t = tot_t = 0.0; gap_rest = 0.0
    for m in ms:
        ns = notes[m]
        t = solos[m]['tempo'] or 0
        b = '<140' if t < 140 else '140-200' if t < 200 else '200-260' if t < 260 else '>260'
        span = ns[-1]['on'] + ns[-1]['dur'] - ns[0]['on']
        bars = span / (4 * st.median(n['bd'] for n in ns))  # 4-beat bar equivalents
        for key in (b, 'all'):
            x = bins[key]; x[0] += len(ns); x[1] += bars; x[2] += span; x[3] += 1
        # sounding union
        snd = 0.0; cur_s = ns[0]['on']; cur_e = ns[0]['on'] + ns[0]['dur']
        for n in ns[1:]:
            if n['on'] > cur_e:
                snd += cur_e - cur_s; cur_s = n['on']
            cur_e = max(cur_e, n['on'] + n['dur'])
        snd += cur_e - cur_s
        tot_t += span; rest_t += span - snd
        phs = phrases_of(m)
        for a, bb in zip(phs, phs[1:]):
            gap_rest += max(0.0, bb[0]['on'] - (a[-1]['on'] + a[-1]['dur']))
    R['density'] = dict(bins)
    R['rest_share'] = (100 * rest_t / tot_t, 100 * gap_rest / tot_t)
    # b. phrases
    plen_n, plen_b, prest = [], [], []
    start_beat = Counter(); end_beat = Counter(); start_on = Counter(); end_on = Counter()
    antic = cross = 0; nph = nbar = 0
    for m in ms:
        phs = phrases_of(m)
        for k, ph in enumerate(phs):
            nph += 1
            f, l = ph[0], ph[-1]
            plen_n.append(len(ph))
            plen_b.append(l['pos'] - f['pos'] + l['durb'])
            if f['num'] == 1: continue  # 1/4-notated (free meter) solos: no bar position
            nbar += 1
            start_beat[f['beat']] += 1; end_beat[l['beat']] += 1
            start_on['on' if f['tat'] == 1 else 'off'] += 1
            end_on['on' if l['tat'] == 1 else 'off'] += 1
            if f['beat'] == f['num'] and f['tat'] != 1: antic += 1
            if f['beat'] >= f['num'] - 1 and len(ph) > 1 and any(n['bar'] > f['bar'] for n in ph[1:3]): cross += 1
            if k + 1 < len(phs):
                prest.append(phs[k + 1][0]['pos'] - (l['pos'] + l['durb']))
    R['phr'] = dict(n=nph, len_n=plen_n, len_b=plen_b, rest=prest, sb=start_beat, eb=end_beat, so=start_on, eo=end_on,
                    antic=antic, cross=cross, nbar=nbar)
    # c. rhythm (IOIs within phrases, metrical grid)
    rc = Counter(); runs = []
    for m in ms:
        for ph in phrases_of(m):
            run = best = 0
            for a, b in zip(ph, ph[1:]):
                cl = ioi_class(b['pos'] - a['pos'])
                rc[cl] += 1
                if cl == '8th':
                    run += 1; best = max(best, run)
                else:
                    run = 0
            runs.append(best + 1 if best else 0)
    R['rhythm'] = (rc, runs)
    # d. pitch vs harmony
    cl = {k: Counter() for k in ('all', 'strong', 'onbeat', 'offbeat', 'pfinal')}
    for m in ms:
        for n in notes[m]:
            if n['cls'] is None: continue
            cl['all'][n['cls']] += 1
            if n['tat'] == 1:
                cl['onbeat'][n['cls']] += 1
                if n['beat'] in (1, 3) and n['num'] in (4, 6): cl['strong'][n['cls']] += 1
            else:
                cl['offbeat'][n['cls']] += 1
            if n['pfinal']: cl['pfinal'][n['cls']] += 1
    R['harm'] = cl
    # e. intervals
    ivc = Counter(); leapn = leapback = 0; ranges = []; reg = defaultdict(list)
    for m in ms:
        for n in notes[m]: reg[solos[m]['inst']].append(n['p'])
        for ph, ivs in intervals_in_phrases(m):
            ranges.append(max(n['p'] for n in ph) - min(n['p'] for n in ph))
            for i, iv in enumerate(ivs):
                ivc[iv] += 1
                if abs(iv) >= 5 and i + 1 < len(ivs):
                    leapn += 1
                    nx = ivs[i + 1]
                    if 1 <= abs(nx) <= 2 and (nx > 0) != (iv > 0): leapback += 1
    R['iv'] = (ivc, leapn, leapback, ranges, reg)
    # f. approach habits + g. patterns
    tgt = hb = ha = enc = 0; chrom = chrom_other = tot_mid = 0
    pat = Counter(); npat = 0; pen = pen4 = dia = win = 0
    for m in ms:
        for ph in phrases_of(m):
            for i, n in enumerate(ph):
                if i >= 1 and n['tat'] == 1 and n['beat'] in (1, 3) and n['num'] in (4, 6) and n['cls'] in CHORD_TONES:
                    tgt += 1
                    d = ph[i - 1]['p'] - n['p']
                    if d == -1: hb += 1
                    if d == 1: ha += 1
                    if i >= 2:
                        d2 = ph[i - 2]['p'] - n['p']
                        if (1 <= d <= 2 and -2 <= d2 <= -1) or (1 <= d2 <= 2 and -2 <= d <= -1): enc += 1
                if 1 <= i < len(ph) - 1:
                    tot_mid += 1
                    a = n['p'] - ph[i - 1]['p']; b = ph[i + 1]['p'] - n['p']
                    if abs(a) == 1 and a == b:
                        chrom += 1
                        if n['cls'] == 'other': chrom_other += 1
            ps = [n['p'] for n in ph]
            for i in range(len(ps) - 3):
                w = ps[i:i + 4]
                pat[(w[1] - w[0], w[2] - w[1], w[3] - w[2])] += 1; npat += 1
                pcs = frozenset(x % 12 for x in w)
                if len(pcs) >= 3:
                    win += 1
                    if any(pcs <= S for S in PENTA):
                        pen += 1
                        if len(pcs) == 4: pen4 += 1
                    if any(pcs <= S for S in DIATONIC): dia += 1
    R['appr'] = (tgt, hb, ha, enc, chrom, chrom_other, tot_mid)
    R['pat'] = (pat, npat, pen, pen4, dia, win)
    # h. midlevel
    mu = Counter(); mun = Counter()
    for m in ms:
        for s, e, v in sections[m]['IDEA']:
            mm = re.search(r'(lick|line|melody|rhythm|expressive|quote|void|theme|fragment|oscillation)', v)
            k = mm.group(1) if mm else '?'
            mu[k] += 1; mun[k] += e - s + 1
    R['mid'] = (mu, mun)
    return R

order = list(G)
RS = {g: stats(G[g]) for g in order}

def table(title, cols, rows):
    print('\n== ' + title)
    w0 = max(len(r[0]) for r in rows) + 1
    ws = [max(len(str(c)), max(len(str(r[i + 1])) for r in rows)) for i, c in enumerate(cols)]
    print(' ' * w0 + ' '.join(str(c).rjust(w) for c, w in zip(cols, ws)))
    for r in rows:
        print(r[0].ljust(w0) + ' '.join(str(x).rjust(w) for x, w in zip(r[1:], ws)))

print('Weimar Jazz Database solos. n = (solos, notes) per group.')
table('groups', ['solos', 'notes', 'years'], [(g, RS[g]['n'][0], RS[g]['n'][1],
      f"{min(yr(m) for m in G[g])}-{max(yr(m) for m in G[g])}") for g in order])

# a
rows = []
for g in order:
    d = RS[g]['density']; r = [g]
    for b in ['all', '<140', '140-200', '200-260', '>260']:
        x = d.get(b)
        r.append(f"{x[0]/x[1]:.2f}/{x[0]/x[2]:.2f} ({x[3]})" if x else '-')
    rows.append(r)
table('a. density: notes per 4-beat bar / notes per sec (n solos), by avg tempo bin (bpm)', ['all', '<140', '140-200', '200-260', '>260'], rows)
table('a. rest share of solo time (%): silent time (between note offsets & next onsets) / between-phrase gaps only',
      ['silent%', 'phrase-gap%'], [(g, f"{RS[g]['rest_share'][0]:.1f}", f"{RS[g]['rest_share'][1]:.1f}") for g in order])

# b
rows = []
for g in order:
    p = RS[g]['phr']
    rows.append((g, p['n'], miqr(p['len_n']), miqr(p['len_b']), miqr(p['rest']), pct(p['antic'], p['nbar']), pct(p['cross'], p['nbar'])))
table('b. phrases: n, length notes med [IQR], length beats, rest before next phrase (beats), % start on the "and" of the last beat (anticipation), % pickup crossing bar line within 2 notes',
      ['n phr', 'len notes', 'len beats', 'rest beats', 'antic%', 'pickup%'], rows)
rows = []
for g in order:
    p = RS[g]['phr']; n = p['nbar']
    rows.append((g,) + tuple(pct(p['sb'][b], n) for b in (1, 2, 3, 4)) + (pct(p['so']['on'], n),) +
                tuple(pct(p['eb'][b], n) for b in (1, 2, 3, 4)) + (pct(p['eo']['on'], n),))
table('b. phrase start / end position in bar (% of phrases): beat 1..4, % on the beat (rest are off-beat)',
      ['st b1', 'st b2', 'st b3', 'st b4', 'st on', 'end b1', 'end b2', 'end b3', 'end b4', 'end on'], rows)

# c
rows = []
for g in order:
    rc, runs = RS[g]['rhythm']; t = sum(rc.values())
    rows.append((g, t) + tuple(pct(rc[k], t) for k in RCLASSES) + (f"{q(runs,.5):.0f}", f"{q(runs,.9):.0f}"))
table('c. rhythm: inter-onset intervals within phrases on the metrical grid (% of IOIs); longest 8th run per phrase in notes (med, p90; 0 = none)',
      ['n IOI'] + RCLASSES + ['8run med', '8run p90'], rows)
rows = []
for g in order:
    durb = [n['durb'] for m in G[g] for n in notes[m]]
    rows.append((g, miqr(durb)))
table('c. sounding duration in beats (med [IQR], articulation, not grid)', ['dur beats'], rows)

# d
for sub, lab in [('all', 'all notes'), ('strong', 'on beats 1&3'), ('onbeat', 'on any beat'), ('offbeat', 'off-beat'), ('pfinal', 'phrase-final notes')]:
    rows = []
    for g in order:
        cl = RS[g]['harm'][sub]; t = sum(cl.values())
        ct = sum(cl[k] for k in CHORD_TONES)
        rows.append((g, t, pct(ct, t)) + tuple(pct(cl[k], t) for k in CLASSES))
    table(f'd. chordal-diatonic class, {lab} (% of notes with a chord; 6/13 = major 6th above root; other = chromatic, e.g. maj3 on minor, maj7 on dom, b7 on maj7)',
          ['n', 'CT(1357)'] + CLASSES, rows)

# e
rows = []
IVB = list(range(-12, 13))
for g in order:
    ivc, ln, lb, rng, reg = RS[g]['iv']; t = sum(ivc.values())
    rep = ivc[0]; step = sum(ivc[i] for i in (-2, -1, 1, 2)); leap = sum(v for k, v in ivc.items() if abs(k) >= 5)
    up = sum(v for k, v in ivc.items() if k > 0)
    rows.append((g, t, pct(rep, t), pct(step, t), pct(sum(ivc[i] for i in (-4, -3, 3, 4)), t), pct(leap, t), pct(up, t),
                 pct(lb, ln), f"{st.mean(rng):.1f}", miqr(rng)))
table('e. intervals within phrases: % repeated, % step (1-2), % third (3-4), % leap (>=5), % upward, % leaps followed by a step back (opposite dir), phrase range semitones mean, med [IQR]',
      ['n iv', 'rep%', 'step%', '3rd%', 'leap%', 'up%', 'leap->stepback%', 'range mean', 'range med'], rows)
rows = []
for g in order:
    ivc = RS[g]['iv'][0]; t = sum(ivc.values())
    rows.append((g, pct(sum(v for k, v in ivc.items() if k < -12), t)) + tuple(pct(ivc[i], t) for i in IVB) +
                (pct(sum(v for k, v in ivc.items() if k > 12), t),))
table('e. interval distribution (% of intervals within phrases), semitones', ['<-12'] + [f"{i:+d}" for i in IVB] + ['>12'], rows)
rows = []
for g in order:
    reg = RS[g]['iv'][4]
    for inst, ps in sorted(reg.items()):
        rows.append((f"{g} [{inst}]", len(ps), miqr(ps), min(ps), max(ps)))
table('e. register: MIDI pitch per instrument (n notes, med [IQR], min, max)', ['n', 'pitch', 'min', 'max'], rows)

# f
rows = []
for g in order:
    tgt, hb, ha, enc, ch, cho, tm = RS[g]['appr']
    rows.append((g, tgt, pct(hb, tgt), pct(ha, tgt), pct(hb + ha, tgt), pct(enc, tgt), pct(ch, tm), pct(cho, tm)))
table('f. approach: chord tones (1/3/5/7) on beats 1&3 (n), % approached by half step from below / above / either, % by enclosure (prev 2 notes = one 1-2 st above + one 1-2 st below, either order); % of inner notes that are chromatic passing (half step in & out, same direction), and of those with a chromatic (other) class',
      ['n tgt', 'hs below', 'hs above', 'hs any', 'enclosure', 'chrom pass', 'chrom pass(other)'], rows)

# g
for g in order:
    pat, npat, pen, pen4, dia, win = RS[g]['pat']
    s = '; '.join(f"({','.join(f'{x:+d}' for x in k)}) {100*v/npat:.2f}" for k, v in pat.most_common(10))
    print(f"\n== g. {g}: n windows {npat}; top-10 4-note interval patterns (%): {s}")
rows = []
for g in order:
    pat, npat, pen, pen4, dia, win = RS[g]['pat']
    top10 = sum(v for k, v in pat.most_common(10))
    rows.append((g, win, pct(pen, win), pct(pen4, win), pct(dia, win), pct(top10, npat), len(pat)))
table('g. 4-note windows with >=3 distinct pitch classes: % inside one pentatonic (maj/min same sets), % with 4 distinct pcs all pentatonic, % inside one diatonic scale (baseline); share of windows covered by top 10 interval patterns; distinct patterns',
      ['n win', 'penta%', 'penta 4pc%', 'diatonic%', 'top10 cover%', 'distinct'], rows)

# h
MU = ['lick', 'line', 'melody', 'rhythm', 'expressive', 'theme', 'quote', 'fragment', 'void', 'oscillation']
rows = []
for g in order:
    mu, mun = RS[g]['mid']; t = sum(mu.values()); tn = sum(mun.values())
    rows.append((g, t) + tuple(f"{pct(mu[k], t)}/{pct(mun[k], tn)}" for k in MU))
table('h. midlevel (IDEA) units: % of units / % of notes covered', ['n units'] + MU, rows)
