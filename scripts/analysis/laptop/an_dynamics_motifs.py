import sqlite3, sys, re, random, math, statistics as st
from collections import defaultdict, Counter

DB = sys.argv[1]
c = sqlite3.connect(DB)
random.seed(1)

# ---------- load ----------
meta = {}
for melid, perf, inst, style, rd, title in c.execute(
        'select s.melid,s.performer,s.instrument,s.style,t.recordingdate,s.title from solo_info s join track_info t using(trackid)'):
    y = re.findall(r'(19\d\d|20\d\d)', rd or '')
    meta[melid] = dict(perf=perf, inst=inst, style=style, year=int(y[0]) if y else None, title=title)

notes = defaultdict(list)
for r in c.execute('select melid,onset,pitch,duration,division,bar,beat,tatum,num,denom,beatdur,loud_med,loud_max,loud_sd,loud_relpos,loud_cent,loud_s2b,f0_mod from melody order by eventid'):
    m = r[0]
    notes[m].append(dict(on=r[1], p=r[2], dur=r[3], div=r[4], bar=r[5], beat=r[6], tat=r[7], num=r[8], den=r[9],
                         bd=r[10], lm=r[11] if (r[11] is not None and r[11] > 20) else None, lmax=r[12], lsd=r[13], lrp=r[14], lc=r[15], s2b=r[16], f0=r[17]))

secs = defaultdict(lambda: defaultdict(list))
for m, t, s, e, v in c.execute('select melid,type,start,end,value from sections order by melid,start'):
    secs[m][t].append((s, e, v))

bars = defaultdict(dict)  # melid -> bar -> (chorus_id, formlabel)
for m, bar, cid, form in c.execute('select melid,bar,chorus_id,form from beats order by beatid'):
    d = bars[m]
    if bar not in d:
        d[bar] = [cid, form]
    elif form and not d[bar][1]:
        d[bar][1] = form
for m, d in bars.items():
    cur = ''
    for b in sorted(d):
        if d[b][1]:
            cur = d[b][1]
        d[b][1] = cur

print('notes with loud_med missing or at the -66 dB failure floor (excluded from loudness):',
      sum(1 for ns in notes.values() for n in ns if n['lm'] is None))
# per-note derived
for m, ns in notes.items():
    vals = [n['lm'] for n in ns if n['lm'] is not None]
    med = st.median(vals)
    for i, n in enumerate(ns):
        n['i'] = i
        n['rel'] = None if n['lm'] is None else n['lm'] - med
        n['chord'] = None
    for s, e, v in secs[m]['CHORD']:
        for i in range(s, min(e, len(ns) - 1) + 1):
            ns[i]['chord'] = v
    for s, e, v in secs[m]['PHRASE']:
        assert e < len(ns)

def groups(m):
    md = meta[m]; p, y = md['perf'], md['year']
    g = ['ALL', 'STYLE:' + md['style']]
    if p == 'Miles Davis':
        if 1951 <= y <= 1959: g.append('Miles 51-59')
        if 1964 <= y <= 1969: g.append('Miles 64-69')
    if p == 'John Coltrane':
        g.append('Coltrane <1960' if y < 1960 else 'Coltrane 1960+')
    for name, key in [('Charlie Parker', 'Parker'), ('Sonny Rollins', 'Rollins'), ('Wayne Shorter', 'Shorter'),
                      ('Louis Armstrong', 'Armstrong')]:
        if p == name: g.append(key)
    if y and y <= 1930: g.append('pre-1931 all')
    return g

GORDER = ['Miles 51-59', 'Miles 64-69', 'Coltrane <1960', 'Coltrane 1960+', 'Parker', 'Rollins', 'Shorter',
          'Armstrong', 'pre-1931 all', 'ALL', 'STYLE:TRADITIONAL', 'STYLE:SWING', 'STYLE:BEBOP', 'STYLE:COOL',
          'STYLE:HARDBOP', 'STYLE:POSTBOP', 'STYLE:FUSION', 'STYLE:FREE']
SHORT = {g: g.replace('STYLE:', '') for g in GORDER}

def mean(x): return sum(x) / len(x) if x else float('nan')
def med(x): return st.median(x) if x else float('nan')
def q(x, p):
    if not x: return float('nan')
    x = sorted(x); k = (len(x) - 1) * p; f = math.floor(k); cc = min(f + 1, len(x) - 1)
    return x[f] + (x[cc] - x[f]) * (k - f)
def corr(a, b):
    n = len(a)
    if n < 3: return float('nan')
    ma, mb = mean(a), mean(b)
    sab = sum((x - ma) * (y - mb) for x, y in zip(a, b)); saa = sum((x - ma) ** 2 for x in a); sbb = sum((y - mb) ** 2 for y in b)
    return sab / math.sqrt(saa * sbb) if saa > 0 and sbb > 0 else float('nan')
def slope(a, b):
    ma, mb = mean(a), mean(b)
    saa = sum((x - ma) ** 2 for x in a)
    return sum((x - ma) * (y - mb) for x, y in zip(a, b)) / saa if saa else float('nan')

A = defaultdict(lambda: defaultdict(list))  # A[group][key] -> list of values
def add(m, key, v):
    for g in groups(m): A[g][key].append(v)

def table(title, cols, fmt='{:+.2f}'):
    """cols: list of (header, func(group)->(value, n) or str)"""
    print('\n### ' + title)
    hdr = '%-16s' % 'group' + ''.join('%-18s' % h for h, _ in cols)
    print(hdr)
    for g in GORDER:
        if g not in A: continue
        row = '%-16s' % SHORT[g]
        for h, f in cols:
            v = f(g)
            if isinstance(v, tuple):
                val, n = v
                s = ('nan' if val != val else fmt.format(val)) + ' (%d)' % n
            else:
                s = str(v)
            row += '%-18s' % s
        print(row)

def M(key, fn=mean):  # column helper: mean of list with n
    return lambda g: (fn(A[g][key]), len(A[g][key]))
def SH(key):  # share of truthy values
    return lambda g: (100 * mean([1 if x else 0 for x in A[g][key]]), len(A[g][key]))

def phrases(m):
    ns = notes[m]
    return [ns[s:e + 1] for s, e, v in secs[m]['PHRASE']]

# ================= performers =================
print('## PERFORMERS (performer | instrument | solos | years | styles)')
pc = defaultdict(list)
for m, md in meta.items(): pc[(md['perf'], md['inst'])].append(md)
for (p, i), l in sorted(pc.items()):
    ys = [x['year'] for x in l if x['year']]
    print('%s | %s | %d | %s-%s | %s' % (p, i, len(l), min(ys), max(ys), ','.join(sorted(set(x['style'] for x in l)))))
print('\nGroup sizes (solos / notes):')
gs = defaultdict(lambda: [0, 0])
for m in meta:
    for g in groups(m): gs[g][0] += 1; gs[g][1] += len(notes[m])
for g in GORDER: print('  %-16s %4d solos %7d notes' % (SHORT[g], gs[g][0], gs[g][1]))

# ================= 3. DYNAMICS =================
for m, ns in notes.items():
    rel = [n['rel'] for n in ns if n['rel'] is not None]
    add(m, 'solo_sd', st.pstdev(rel))
    add(m, 'solo_p1090', q(rel, .9) - q(rel, .1))
    add(m, 'solo_absmed', st.median([n['lm'] for n in ns if n['lm'] is not None]))
    four = all(n['num'] == 4 and n['den'] == 4 for n in ns[:5])
    # metric
    for k, n in enumerate(ns):
        if n['rel'] is None: continue
        if n['tat'] == 1:
            add(m, 'onbeat', n['rel'])
            if n['num'] == 4 and 1 <= n['beat'] <= 4: add(m, 'beat%d' % n['beat'], n['rel'])
        elif n['div'] == 2 and n['tat'] == 2:
            add(m, 'off8', n['rel'])
            pv = ns[k - 1] if k else None
            if pv and pv['rel'] is not None and pv['bar'] == n['bar'] and pv['beat'] == n['beat'] and pv['tat'] == 1:
                d = n['rel'] - pv['rel']
                add(m, 'off_minus_on', d); add(m, 'off_louder', d > 0); add(m, 'off_louder1', d > 1); add(m, 'off_ghost3', d < -3)
        elif n['div'] == 4 and n['tat'] == 3:
            add(m, 'off16and', n['rel'])
        else:
            add(m, 'othermetric', n['rel'])
        # duration
        db = n['dur'] / n['bd'] if n['bd'] else None
        if db is not None:
            b = 'd<0.3' if db < .3 else 'd0.3-0.6' if db < .6 else 'd0.6-1.2' if db < 1.2 else 'd1.2-2.5' if db < 2.5 else 'd>2.5'
            add(m, b, n['rel'])
        # articulation
        if n['lrp'] is not None: add(m, 'lrp', n['lrp'])
        if n['lc'] is not None: add(m, 'lcent', n['lc'])
        if n['s2b'] is not None: add(m, 's2b', n['s2b'])
        if n['lsd'] is not None: add(m, 'lsd', n['lsd'])
        if n['lmax'] is not None and n['lmax'] > 0: pass
    for n in ns:
        for t in ('vibrato', 'slide', 'bend', 'fall-off'): add(m, 'f0_' + t, n['f0'] == t)
    # phrases
    phs = phrases(m)
    prev_end = None
    for pi, ph in enumerate(phs):
        r = [n['rel'] for n in ph]
        ok = all(x is not None for x in r)
        if ok and len(ph) >= 5:
            pm = mean(r)
            L = len(ph)
            for k, n in enumerate(ph):
                if k == 0: b = 'first'
                elif k == L - 1: b = 'last'
                else:
                    pos = k / (L - 1)
                    b = 'early' if pos < 1 / 3 else 'middle' if pos < 2 / 3 else 'late'
                add(m, 'arch_' + b, n['rel']); add(m, 'archc_' + b, n['rel'] - pm)
            mx = max(n['p'] for n in ph)
            hi = [n['rel'] for n in ph if n['p'] == mx]; lo = [n['rel'] for n in ph if n['p'] != mx]
            if lo: add(m, 'hi_minus_rest', mean(hi) - mean(lo))
            # pitch vs loudness centred
            pp = [n['p'] for n in ph]; mp = mean(pp)
            for n in ph:
                add(m, 'pl_pair', (n['p'] - mp, n['rel'] - pm))
            if len(ph) >= 6:
                cc = corr(pp, r)
                if cc == cc: add(m, 'pl_corr_phrase', cc)
            # final notes
            add(m, 'final_minus_mean', ph[-1]['rel'] - pm)
            add(m, 'final_dur', ph[-1]['dur'] / ph[-1]['bd'])
            add(m, 'nonfinal_dur', med([n['dur'] / n['bd'] for n in ph[:-1]]))
        # rest before phrase
        if ok and prev_end is not None and len(ph) >= 3:
            gap = (ph[0]['on'] - prev_end) / ph[0]['bd']
            b = 'g<0.5' if gap < .5 else 'g0.5-1' if gap < 1 else 'g1-2' if gap < 2 else 'g2-4' if gap < 4 else 'g>4'
            add(m, 'phr_' + b, mean(r)); add(m, 'phr3_' + b, mean(r[:3]))
        prev_end = ph[-1]['on'] + ph[-1]['dur']
    # chorus
    bd = bars[m]
    chbars = defaultdict(list)
    for b, (cid, f) in bd.items():
        if cid and cid >= 1: chbars[cid].append(b)
    nch = max(chbars) if chbars else 0
    for n in ns:
        if n['rel'] is None or n['bar'] not in bd: continue
        cid, f = bd[n['bar']]
        if not cid or cid < 1: continue
        add(m, 'chorus_' + ('1' if cid == 1 else '2' if cid == 2 else '3' if cid == 3 else '4+'), n['rel'])
        if nch >= 2 and cid == nch: add(m, 'chorus_last', n['rel'])
        if nch >= 2 and 1 < cid < nch: add(m, 'chorus_mid', n['rel'])
        L = len(chbars[cid]); rb = n['bar'] - min(chbars[cid])
        if L == 32: add(m, 'sec32_%d' % (rb // 8 + 1), n['rel'])
        if L == 32 and f: add(m, 'form32_' + f[0], n['rel'])
        if L == 12: add(m, 'sec12_%d' % (rb // 4 + 1), n['rel'])

print('\n\n## 3. DYNAMICS  (loud_med dB minus the solo median; n in parentheses)')
table('3a spread per solo (n = solos)', [('SD dB', M('solo_sd')), ('p10-p90 dB', M('solo_p1090')),
      ('SD median', M('solo_sd', med)), ('abs med loud dB', M('solo_absmed'))], '{:.2f}')
table('3b phrase arch, solo-relative dB (phrases>=5 notes; n = notes)',
      [(b, M('arch_' + b)) for b in ('first', 'early', 'middle', 'late', 'last')])
table('3b phrase arch, phrase-centred dB (n = notes)',
      [(b, M('archc_' + b)) for b in ('first', 'early', 'middle', 'late', 'last')])
table('3b highest note minus rest of phrase (n = phrases)', [('dB mean', M('hi_minus_rest')), ('median', M('hi_minus_rest', med)),
      ('share >0 %', lambda g: (100 * mean([x > 0 for x in A[g]['hi_minus_rest']]), len(A[g]['hi_minus_rest'])))])
def plc(g):
    pr = A[g]['pl_pair']; return (corr([a for a, b in pr], [b for a, b in pr]), len(pr))
def pls(g):
    pr = A[g]['pl_pair']; return (12 * slope([a for a, b in pr], [b for a, b in pr]), len(pr))
table('3c pitch vs loudness within phrases', [('pooled r', plc), ('dB/octave', pls),
      ('mean per-phrase r', M('pl_corr_phrase')), ('median r', M('pl_corr_phrase', med))])
table('3d metric position (n = notes)', [('on-beat', M('onbeat')), ('swung off8 d2t2', M('off8')),
      ('16th "and" d4t3', M('off16and')), ('other', M('othermetric'))])
table('3d on-beat notes by beat (4/4)', [('beat%d' % b, M('beat%d' % b)) for b in (1, 2, 3, 4)])
table('3d offbeat 8th vs preceding on-beat note of the same beat (n = pairs)',
      [('off-on dB mean', M('off_minus_on')), ('median', M('off_minus_on', med)), ('off louder %', SH('off_louder')),
       ('off >+1dB %', SH('off_louder1')), ('off <-3dB ghost %', SH('off_ghost3'))])
table('3e duration in beats vs loudness (n = notes)', [(b, M(b)) for b in ('d<0.3', 'd0.3-0.6', 'd0.6-1.2', 'd1.2-2.5', 'd>2.5')])
table('3e phrase-final note (n = phrases)', [('final-phr mean dB', M('final_minus_mean')), ('final dur beats med', M('final_dur', med)),
      ('nonfinal dur med', M('nonfinal_dur', med))])
table('3f by chorus number (n = notes)', [(b, M('chorus_' + b)) for b in ('1', '2', '3', '4+', 'mid', 'last')])
table('3f 32-bar choruses by 8-bar section (n = notes)', [('bars%d' % (8 * k - 7), M('sec32_%d' % k)) for k in (1, 2, 3, 4)])
table('3f 32-bar choruses by FORM label letter (n = notes)', [(k, M('form32_' + k)) for k in ('A', 'B', 'C')])
table('3f 12-bar choruses by 4-bar line (n = notes)', [('bars%d' % (4 * k - 3), M('sec12_%d' % k)) for k in (1, 2, 3)])
table('3f phrase mean loudness by rest before it, in beats (n = phrases)', [(b, M('phr_' + b)) for b in ('g<0.5', 'g0.5-1', 'g1-2', 'g2-4', 'g>4')])
table('3f first-3-notes loudness by rest before phrase (n = phrases)', [(b, M('phr3_' + b)) for b in ('g<0.5', 'g0.5-1', 'g1-2', 'g2-4', 'g>4')])
table('3g articulation proxies (n = notes)', [('loud_relpos', M('lrp')), ('loud_cent', M('lcent')), ('loud_sd', M('lsd')),
      ('loud_s2b', M('s2b'))], '{:.3f}')
table('3g f0 modulation share % (n = notes)', [(t, SH('f0_' + t)) for t in ('vibrato', 'slide', 'bend', 'fall-off')], '{:.2f}')

# ================= 4. MOTIFS =================
def ioic(x):
    return 0 if x < .3 else 1 if x < .75 else 2 if x < 1.5 else 3 if x < 3 else 4
def lev(a, b):
    if len(a) < len(b): a, b = b, a
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]
def nsim(a, b):
    return 1 - lev(a, b) / max(len(a), len(b))

class Ph:
    def __init__(s, ns):
        s.ns = ns
        s.iv = tuple(int(round(ns[k + 1]['p'] - ns[k]['p'])) for k in range(len(ns) - 1))
        s.rh = tuple(ioic((ns[k + 1]['on'] - ns[k]['on']) / ns[k]['bd']) for k in range(len(ns) - 1))
        s.p0 = ns[0]['p']; s.on = ns[0]['on']; s.end = ns[-1]['on'] + ns[-1]['dur']; s.bd = mean([n['bd'] for n in ns])

def classify(a, b):
    """a = earlier, b = later. Thresholds: near = interval similarity >= 0.7 (normalised Levenshtein)."""
    si, sr = nsim(a.iv, b.iv), nsim(a.rh, b.rh)
    if a.iv == b.iv:
        if a.p0 != b.p0: return 'transposed', si, sr
        return ('exact' if a.rh == b.rh else 'pitch-rep/rhythm-altered'), si, sr
    sh, lo = (a.iv, b.iv) if len(a.iv) < len(b.iv) else (b.iv, a.iv)
    if len(sh) < len(lo) and len(sh) >= 3:
        tol = min(2, len(sh) // 3)
        if lev(sh, lo[:len(sh)]) <= tol or lev(sh, lo[-len(sh):]) <= tol:
            return 'trunc/extended', si, sr
    if si >= .7: return 'varied (int sim>=.7)', si, sr
    if a.rh == b.rh and len(a.rh) >= 3: return 'rhythm-rep/new pitches', si, sr
    return 'unrelated', si, sr
CATS = ['exact', 'transposed', 'pitch-rep/rhythm-altered', 'trunc/extended', 'varied (int sim>=.7)', 'rhythm-rep/new pitches', 'unrelated']

def internal_seq(iv, p, L, strict, adjacent):
    """repeated cell of L intervals (L+1 notes), non-overlapping; returns list of transpositions"""
    out = []
    for i in range(len(iv) - L + 1):
        cell = iv[i:i + L]
        if len(set(cell)) == 1: continue
        if strict and not (any(abs(x) >= 3 for x in cell) or any((x > 0) != (y > 0) for x, y in zip(cell, cell[1:]) if x and y)):
            continue
        jr = range(i + L, min(i + L + 2, len(iv) - L + 1)) if adjacent else range(i + L, len(iv) - L + 1)
        for j in jr:
            if iv[j:j + L] == cell:
                out.append(int(p[j] - p[i])); break
        if out: break
    return out

for m, ns in notes.items():
    phs = [Ph(ph) for ph in phrases(m) if len(ph) >= 4]
    allp = phrases(m)
    for k, ph in enumerate(phs):
        # 4b internal sequences
        pit = [n['p'] for n in ph.ns]
        for L in (2, 3, 4):
            add(m, 'seq_any_%d' % L, bool(internal_seq(ph.iv, pit, L, False, False)))
            add(m, 'seq_strict_%d' % L, bool(internal_seq(ph.iv, pit, L, True, False)))
            t = internal_seq(ph.iv, pit, L, True, True)
            add(m, 'seq_adj_%d' % L, bool(t))
            if t: add(m, 'seq_tr_%d' % L, t[0])
            sh = list(ph.iv); random.shuffle(sh)
            add(m, 'seq_shuf_strict_%d' % L, bool(internal_seq(tuple(sh), pit, L, True, False)))
            add(m, 'seq_shuf_adj_%d' % L, bool(internal_seq(tuple(sh), pit, L, True, True)))
    # pairs
    rel1 = []
    for lag in (1, 2):
        for k in range(lag, len(phs)):
            a, b = phs[k - lag], phs[k]
            cat, si, sr = classify(a, b)
            add(m, 'cat%d' % lag, cat); add(m, 'si%d' % lag, si); add(m, 'sr%d' % lag, sr)
            if lag == 1: rel1.append(cat != 'unrelated')
            if cat != 'unrelated':
                add(m, 'dist%d' % lag, (b.on - a.on) / b.bd); add(m, 'gapr%d' % lag, (b.on - a.end) / b.bd)
            # start/end matching (4d)
            if a.iv != b.iv:
                st3 = a.iv[:3] == b.iv[:3]; en3 = a.iv[-3:] == b.iv[-3:]
                add(m, 'start3_%d' % lag, st3); add(m, 'end3_%d' % lag, en3)
                if st3:
                    cp = 0
                    while cp < min(len(a.iv), len(b.iv)) and a.iv[cp] == b.iv[cp]: cp += 1
                    full = cp == min(len(a.iv), len(b.iv))
                    add(m, 'div%d' % lag, 'prefix(one contains other)' if full else ('diverge@%d' % (cp + 2) if cp + 2 <= 6 else 'diverge@7+'))
                    add(m, 'div_rhythm_same%d' % lag, a.rh[:cp] == b.rh[:cp])
    # baseline: random non-near phrase in same solo
    for k in range(len(phs)):
        cand = [j for j in range(len(phs)) if abs(j - k) >= 4]
        if not cand: continue
        j = random.choice(cand); a, b = (phs[j], phs[k]) if j < k else (phs[k], phs[j])
        cat, si, sr = classify(a, b)
        add(m, 'catB', cat); add(m, 'siB', si); add(m, 'srB', sr)
        if a.iv != b.iv:
            add(m, 'start3_B', a.iv[:3] == b.iv[:3]); add(m, 'end3_B', a.iv[-3:] == b.iv[-3:])
    # runs
    run = 0
    for r in rel1 + [False]:
        if r: run += 1
        elif run:
            add(m, 'run', run + 1); run = 0
    add(m, 'nphr_pairs', len(rel1))
    # phrase count stats
    for ph in allp: add(m, 'phrlen', len(ph))

print('\n\n## 4. MOTIFS / REPETITION  (phrases = PHRASE annotations with >= 4 notes)')
print('Intervals: semitones; rhythm: IOI/beat classes <.3 | .3-.75 | .75-1.5 | 1.5-3 | >3 beats.')
print('Pair classes (first match wins): exact = same intervals, start pitch and rhythm classes; transposed = same intervals, other start pitch;')
print(' pitch-rep/rhythm-altered = same intervals+start, rhythm classes differ; trunc/extended = shorter (>=3 intervals) matches the')
print(' longer one\'s prefix or suffix with Levenshtein <= min(2, len/3); varied = normalised interval similarity >= 0.7;')
print(' rhythm-rep = identical rhythm classes (>=3) with new intervals; else unrelated. Baseline B = phrase vs a random phrase >=4 away in the same solo.')
table('phrase length (notes, all phrases)', [('mean', M('phrlen')), ('median', M('phrlen', med))], '{:.1f}')
for lag in ('1', '2', 'B'):
    lab = {'1': 'n vs n-1', '2': 'n vs n-2', 'B': 'baseline random pair'}[lag]
    table('4a categories %s, %% of pairs' % lab,
          [(ct[:17], (lambda ct: lambda g: (100 * mean([x == ct for x in A[g]['cat' + lag]]), len(A[g]['cat' + lag])))(ct)) for ct in CATS], '{:.2f}')
    table('4a similarity distribution %s (normalised Levenshtein, 1 = identical)' % lab,
          [('int mean', M('si' + lag)), ('int p50', M('si' + lag, med)), ('int p90', lambda g, l=lag: (q(A[g]['si' + l], .9), len(A[g]['si' + l]))),
           ('rhy mean', M('sr' + lag)), ('rhy p50', M('sr' + lag, med)), ('rhy p90', lambda g, l=lag: (q(A[g]['sr' + l], .9), len(A[g]['sr' + l])))], '{:.3f}')
table('4b internal repetition in a phrase, % of phrases: any non-constant cell anywhere (3/4/5 notes)',
      [('%d notes' % (L + 1), SH('seq_any_%d' % L)) for L in (2, 3, 4)], '{:.1f}')
table('4b strict cells (contain a leap >=3 st or a direction change), anywhere vs shuffled-interval baseline',
      [('%dn real' % (L + 1), SH('seq_strict_%d' % L)) for L in (2, 3, 4)] + [('%dn shuffled' % (L + 1), SH('seq_shuf_strict_%d' % L)) for L in (2, 3, 4)], '{:.1f}')
table('4b strict cells restated right after (gap <= 1 note) = sequences, real vs shuffled',
      [('%dn real' % (L + 1), SH('seq_adj_%d' % L)) for L in (2, 3, 4)] + [('%dn shuffled' % (L + 1), SH('seq_shuf_adj_%d' % L)) for L in (2, 3, 4)], '{:.1f}')
def trh(g, L):
    t = A[g]['seq_tr_%d' % L]; cn = Counter(t); n = len(t)
    if not n: return '-'
    top = sorted(cn.items(), key=lambda x: -x[1])[:7]
    return ' '.join('%+d:%.0f%%' % (k, 100 * v / n) for k, v in top) + ' (n=%d)' % n
print('\n### 4b transposition of adjacent sequences (semitones: share), 4-note cells and 3-note cells')
for g in GORDER:
    if g in A: print('%-16s 4n: %s\n%-16s 3n: %s' % (SHORT[g], trh(g, 3), '', trh(g, 2)))
def runh(g):
    r = A[g]['run']; npairs = sum(A[g]['nphr_pairs'])
    if not r: return '-'
    cn = Counter(min(x, 5) for x in r)
    return ' '.join('%s:%d' % ('5+' if k == 5 else k, cn[k]) for k in sorted(cn)) + ' | mean %.2f max %d | phrases in chains %.1f%% (pairs %d)' % (
        mean(r), max(r), 100 * sum(r) / (npairs + len(A[g]['run'])), npairs)
print('\n### 4c motif chains: runs of consecutive related phrases (n vs n-1 not unrelated); run length in phrases: count')
for g in GORDER:
    if g in A: print('%-16s %s' % (SHORT[g], runh(g)))
table('4c distance between related phrase starts / rest between them, beats (n = related pairs)',
      [('n-1 start dist p50', M('dist1', med)), ('n-1 rest p50', M('gapr1', med)), ('n-2 start dist p50', M('dist2', med)), ('n-2 rest p50', M('gapr2', med))], '{:.2f}')
table('4d start vs end: share of non-identical pairs whose first 3 / last 3 intervals match (%)',
      [('start n-1', SH('start3_1')), ('end n-1', SH('end3_1')), ('start n-2', SH('start3_2')), ('end n-2', SH('end3_2')),
       ('start base', SH('start3_B')), ('end base', SH('end3_B'))], '{:.2f}')
def divh(g):
    d = A[g]['div1'] + A[g]['div2']; n = len(d)
    if not n: return '-'
    cn = Counter(d)
    keys = ['diverge@5', 'diverge@6', 'diverge@7+', 'prefix(one contains other)']
    return ' '.join('%s:%.0f%%' % (k.replace('diverge', 'div'), 100 * cn[k] / n) for k in keys) + ' (n=%d; rhythm same up to divergence %.0f%%)' % (
        n, 100 * mean(A[g]['div_rhythm_same1'] + A[g]['div_rhythm_same2']))
print('\n### 4d when a phrase restates the previous (n-1 or n-2) start (first 3 intervals = 4 notes), where it diverges (note index of first new pitch)')
for g in GORDER:
    if g in A: print('%-16s %s' % (SHORT[g], divh(g)))

# ---- 4e IDEA labels ----
def parse_idea(v):
    v0 = v.strip()
    d = {}
    d['void_to'] = '->' in v0
    if d['void_to']: v0 = v0.split('->', 1)[1]
    d['star'] = v0.startswith('*'); v0 = v0.lstrip('*')
    d['tilde'] = v0.startswith('~'); v0 = v0.lstrip('~')
    mm = re.match(r'^(#+)(\d*)([+\-=]?)', v0)
    if mm:
        d['ref'] = True; d['refdist'] = int(mm.group(2)) if mm.group(2) else len(mm.group(1)); d['dir'] = mm.group(3)
        v0 = v0[mm.end():]
    else:
        d['ref'] = False; d['refdist'] = 0; d['dir'] = ''
    d['base'] = re.split(r'[_:\s]', v0.lstrip('+-='), 1)[0] or '?'
    return d
for m, ns in notes.items():
    units = secs[m]['IDEA']
    phrase_of = {}
    for pi, (s, e, v) in enumerate(secs[m]['PHRASE']):
        for i in range(s, e + 1): phrase_of[i] = pi
    for ui, (s, e, v) in enumerate(units):
        d = parse_idea(v)
        add(m, 'u_base', d['base']); add(m, 'u_ref', d['ref']); add(m, 'u_tilde', d['tilde'])
        add(m, 'u_ref_far', d['ref'] and d['refdist'] > 1)
        add(m, 'u_ref_tr', d['ref'] and d['dir'] in '+-' and d['dir'] != '')
        if d['ref']: add(m, 'u_refdist', d['refdist']); add(m, 'u_refdir', d['dir'] or 'none')
        for i in range(s, e + 1):
            add(m, 'n_base', d['base'] + ('*' if d['star'] else ''))
        # validation: similarity to referenced unit
        if ui >= 1 and e - s >= 2:
            tgt = ui - (d['refdist'] if d['ref'] else 1)
            if 0 <= tgt < ui:
                ts, te, _ = units[tgt]
                if te - ts >= 2:
                    a = tuple(int(round(ns[k + 1]['p'] - ns[k]['p'])) for k in range(ts, te))
                    b = tuple(int(round(ns[k + 1]['p'] - ns[k]['p'])) for k in range(s, e))
                    add(m, 'val_ref' if d['ref'] else 'val_noref', nsim(a, b))
            # tilde: glued? gap to previous unit, same phrase?
            ps, pe, _ = units[ui - 1]
            gap = (ns[s]['on'] - (ns[pe]['on'] + ns[pe]['dur'])) / ns[s]['bd'] if pe < s else float('nan')
            add(m, 'gap_tilde' if d['tilde'] else 'gap_notilde', gap)
            add(m, 'samephr_tilde' if d['tilde'] else 'samephr_notilde', phrase_of.get(s) == phrase_of.get(pe))
print('\n\n### 4e IDEA (midlevel unit) labels')
def topc(key, k=8):
    def f(g):
        cn = Counter(A[g][key]); n = len(A[g][key])
        return ' '.join('%s:%.1f' % (a, 100 * b / n) for a, b in cn.most_common(k)) + ' (n=%d)' % n
    return f
print('unit types, % of units')
for g in GORDER:
    if g in A: print('%-16s %s' % (SHORT[g], topc('u_base', 10)(g)))
table('reference marks, % of units (n = units)', [('# back-ref', SH('u_ref')), ('ref >1 back', SH('u_ref_far')),
      ('#+/#- transposed', SH('u_ref_tr')), ('~ prefix', SH('u_tilde'))], '{:.1f}')
print('back-reference distance (units back: share) and direction mark')
for g in GORDER:
    if g in A: print('%-16s dist %s | dir %s' % (SHORT[g], topc('u_refdist', 6)(g), topc('u_refdir', 4)(g)))
table('validation: interval similarity to the referenced (or previous) unit; ~ units: rest before and same-phrase share',
      [('sim #-units', M('val_ref')), ('sim others', M('val_noref')), ('gap ~ beats p50', M('gap_tilde', med)),
       ('gap others p50', M('gap_notilde', med)), ('~ same phr %', SH('samephr_tilde')), ('others same phr %', SH('samephr_notilde'))], '{:.2f}')
print('\n### 4f theme quoting: % of notes by unit type (theme* = *theme, starred = marked as from the tune)')
for g in GORDER:
    if g in A:
        cn = Counter(A[g]['n_base']); n = len(A[g]['n_base'])
        th = sum(v for k, v in cn.items() if k.startswith('theme')); mel = sum(v for k, v in cn.items() if k.startswith('melody'))
        qu = sum(v for k, v in cn.items() if k.startswith('quote')); star = sum(v for k, v in cn.items() if k.endswith('*'))
        print('%-16s theme %.2f%%  melody %.2f%%  quote %.2f%%  starred(*) %.2f%%  (n=%d notes)' % (SHORT[g], 100 * th / n, 100 * mel / n, 100 * qu / n, 100 * star / n, n))

# ================= 5. HARMONY =================
PC = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}
def parse_chord(v):
    if not v or v == 'NC': return None
    mm = re.match(r'^([A-G])([b#]?)(.*)$', v.split('/')[0])
    if not mm: return None
    root = (PC[mm.group(1)] + (-1 if mm.group(2) == 'b' else 1 if mm.group(2) == '#' else 0)) % 12
    qq = mm.group(3)
    if qq.startswith('sus'): k = 'sus'
    elif qq.startswith('m7b5'): k = 'hdim'
    elif qq.startswith('o'): k = 'dim'
    elif qq.startswith('-j') or qq in ('-', '-6', '-69'): k = 'mtonic'
    elif qq.startswith('-'): k = 'm7'
    elif qq.startswith('+7') or qq.startswith('+79'): k = 'dom'
    elif qq.startswith('+'): k = 'aug'
    elif qq.startswith('7'): k = 'dom'
    else: k = 'maj'
    return root, k
TONES = {'dom': {0, 4, 7, 10}, 'm7': {0, 3, 7, 10}, 'mtonic': {0, 3, 7, 9, 11}, 'maj': {0, 4, 7, 9, 11}, 'hdim': {0, 3, 6, 10},
         'dim': {0, 3, 6, 9}, 'sus': {0, 5, 7, 10}, 'aug': {0, 4, 8, 11}}
SCALE = {'dom': {0, 2, 4, 5, 7, 9, 10}, 'm7': {0, 2, 3, 5, 7, 9, 10}, 'mtonic': {0, 2, 3, 5, 7, 8, 9, 10, 11},
         'maj': {0, 2, 4, 5, 6, 7, 9, 11}, 'hdim': {0, 1, 2, 3, 5, 6, 8, 10}, 'dim': {0, 2, 3, 5, 6, 8, 9, 11},
         'sus': {0, 2, 4, 5, 7, 9, 10}, 'aug': {0, 2, 4, 6, 8, 10, 11}}
SHIFTS = [1, -1, 2, -2, 3, -3, 6]
for m, ns in notes.items():
    four = [n for n in ns if n['num'] == 4 and n['den'] == 4]
    # notes over chords
    for n in ns:
        pc = parse_chord(n['chord'])
        if not pc: continue
        r, k = pc; iv = int(round(n['p'] - r)) % 12
        cls = 'dom' if k == 'dom' else 'other'
        strong = n['tat'] == 1 and n['num'] == 4 and n['beat'] in (1, 3)
        onb = n['tat'] == 1
        out = iv not in SCALE[k]
        add(m, 'out_all_' + cls, out)
        if strong: add(m, 'out_strong_' + cls, out)
        if onb: add(m, 'out_onb_' + cls, out)
        if not onb: add(m, 'out_offb_' + cls, out)
        if cls == 'dom':
            add(m, 'domcat', 'chord' if iv in (0, 4, 7, 10) else 'nat tension 9/13' if iv in (2, 9) else 'alt b9/#9/#11/b13' if iv in (1, 3, 6, 8) else '11' if iv == 5 else 'maj7')
            if strong: add(m, 'domcat_s', 'chord' if iv in (0, 4, 7, 10) else 'nat tension 9/13' if iv in (2, 9) else 'alt b9/#9/#11/b13' if iv in (1, 3, 6, 8) else '11' if iv == 5 else 'maj7')
    # 2-beat windows
    win = defaultdict(list)
    for n in four: win[(n['bar'], (n['beat'] - 1) // 2)].append(n)
    for key, wn in win.items():
        chs = set(n['chord'] for n in wn)
        if len(wn) < 3 or len(chs) != 1: continue
        pc = parse_chord(wn[0]['chord'])
        if not pc: continue
        r, k = pc
        ivs = [int(round(n['p'] - r)) % 12 for n in wn]
        act = sum(i in TONES[k] for i in ivs); outs = sum(i not in SCALE[k] for i in ivs)
        add(m, 'win_n', 1)
        for s in SHIFTS:
            sh = sum(((i - s) % 12) in TONES[k] for i in ivs)
            ok = sh >= act + 2 and sh / len(ivs) >= .75 and outs / len(ivs) >= .5
            add(m, 'slip_%+d' % s, ok)
        add(m, 'slip_pm1', any(sum(((i - s) % 12) in TONES[k] for i in ivs) >= act + 2 and
                              sum(((i - s) % 12) in TONES[k] for i in ivs) / len(ivs) >= .75 and outs / len(ivs) >= .5 for s in (1, -1)))
    # dominant bars
    bb = defaultdict(list)
    for n in four: bb[n['bar']].append(n)
    for bar, bn in bb.items():
        chs = set(n['chord'] for n in bn)
        if len(bn) < 4 or len(chs) != 1: continue
        pc = parse_chord(bn[0]['chord'])
        if not pc or pc[1] != 'dom': continue
        ivs = set(int(round(n['p'] - pc[0])) % 12 for n in bn)
        alt = len(ivs & {1, 6, 8}); nat = len(ivs & {2, 5, 9})
        add(m, 'tt2', alt >= 2); add(m, 'tt3', alt == 3); add(m, 'nat2', nat >= 2)
        add(m, 'ttsub_tones', {1, 6} <= ivs)  # root + 5th of the tritone-sub chord (Db, Ab over G7)

print('\n\n## 5. HARMONY (chord symbols from CHORD annotations; 4/4 only for windows/bars)')
print('Side-slip window = 2-beat half-bar, one chord, >=3 notes; flagged when the chord shifted by s semitones (same quality) covers')
print(' >= 2 more notes with its chord tones than the real chord, covers >= 75% of the notes, and >= 50% of the notes are outside the')
print(' real chord\'s scale. Shifts of +-2, +-3, 6 are controls for chance.')
table('5a side-slip windows %, by shift (n = windows)', [('%+d' % s, SH('slip_%+d' % s)) for s in SHIFTS] + [('+-1 any', SH('slip_pm1'))], '{:.2f}')
table('5b one-chord dominant 7 bars (>=4 notes): tension sets present in the bar, % (n = bars)',
      [('>=2 of b9,#11,b13', SH('tt2')), ('all 3', SH('tt3')), ('>=2 of 9,11,13 ctrl', SH('nat2')), ('b9+b5 (TT root+5)', SH('ttsub_tones'))], '{:.2f}')
table('5c notes outside the chord scale %, V7 vs other chords (scale: mixolydian / dorian / ionian+#11 ...)',
      [('V7 strong 1&3', SH('out_strong_dom')), ('oth strong', SH('out_strong_other')), ('V7 on-beat', SH('out_onb_dom')),
       ('oth on-beat', SH('out_onb_other')), ('V7 off-beat', SH('out_offb_dom')), ('oth off-beat', SH('out_offb_other'))], '{:.2f}')
print('\n### 5c pitch categories over V7: all notes | strong beats (1, 3)')
for g in GORDER:
    if g in A: print('%-16s %s\n%-16s %s' % (SHORT[g], topc('domcat', 5)(g), '', topc('domcat_s', 5)(g)))

# ================= 4g local motif sharing =================
def lcsub(a, b):
    best = 0; bi = 0
    prev = [0] * (len(b) + 1)
    for i in range(1, len(a) + 1):
        cur = [0] * (len(b) + 1)
        for j in range(1, len(b) + 1):
            if a[i - 1] == b[j - 1]:
                cur[j] = prev[j - 1] + 1
                if cur[j] > best and len(set(a[i - cur[j]:i])) > 1: best = cur[j]
        prev = cur
    return best
random.seed(2)
for m, ns in notes.items():
    phs = [Ph(ph) for ph in phrases(m) if len(ph) >= 4]
    def pairs():
        for k in range(1, len(phs)): yield '1', phs[k - 1], phs[k]
        for k in range(2, len(phs)): yield '2', phs[k - 2], phs[k]
        for k in range(len(phs)):
            cand = [j for j in range(len(phs)) if abs(j - k) >= 4]
            if cand: j = random.choice(cand); yield 'B', phs[min(j, k)], phs[max(j, k)]
    for lag, a, b in pairs():
        li = lcsub(a.iv, b.iv)
        lr = lcsub(tuple(zip(a.iv, a.rh)), tuple(zip(b.iv, b.rh)))
        add(m, 'sh4_' + lag, li >= 4); add(m, 'sh6_' + lag, li >= 6); add(m, 'shr4_' + lag, lr >= 4)
        add(m, 'sim50_' + lag, nsim(a.iv, b.iv) >= .5)
        if len(a.ns) <= 12 and len(b.ns) <= 12:
            cat = classify(a, b)[0]
            add(m, 'scat_' + lag, cat)
            add(m, 'srel_' + lag, cat != 'unrelated')
            add(m, 'ssim50_' + lag, nsim(a.iv, b.iv) >= .5)
            add(m, 'srh_' + lag, a.rh == b.rh)
print('\n### 4g local motif sharing: % of phrase pairs sharing a common interval run (transposition-free, not all-equal intervals)')
table('5-note cell = >=4 shared intervals; 7-note = >=6; "+rhythm" = 5-note cell whose IOI classes also match',
      [(k + ' ' + l, SH(k2 + '_' + l)) for k, k2 in (('5n', 'sh4'), ('7n', 'sh6'), ('5n+rhy', 'shr4')) for l in ('1', '2', 'B')], '{:.1f}')
table('whole-phrase interval similarity >= 0.5, % of pairs', [(l, SH('sim50_' + l)) for l in ('1', '2', 'B')], '{:.1f}')
table('short phrases only (both <= 12 notes): related (any class but unrelated) / sim>=.5 / identical rhythm classes, %',
      [('rel ' + l, SH('srel_' + l)) for l in ('1', '2', 'B')] + [('sim.5 ' + l, SH('ssim50_' + l)) for l in ('1', 'B')] +
      [('rhy= ' + l, SH('srh_' + l)) for l in ('1', 'B')], '{:.1f}')
table('short phrases (both <= 12 notes), n vs n-1 classes %',
      [(ct[:17], (lambda ct: lambda g: (100 * mean([x == ct for x in A[g]['scat_1']]), len(A[g]['scat_1'])))(ct)) for ct in CATS], '{:.1f}')

print('\n### per-solo pitch-loudness r (pooled within phrases), Miles / Coltrane / Parker')
for m in sorted(meta):
    md = meta[m]
    if md['perf'] not in ('Miles Davis', 'John Coltrane', 'Charlie Parker'): continue
    pr = []
    for ph in phrases(m):
        if len(ph) >= 5 and all(n['rel'] is not None for n in ph):
            mp = mean([n['p'] for n in ph]); ml = mean([n['rel'] for n in ph])
            pr += [(n['p'] - mp, n['rel'] - ml) for n in ph]
    if len(pr) > 30:
        print('  %-14s %-4s %d %-28s r=%+.2f dB/oct=%+.1f n=%d' % (md['perf'], md['inst'], md['year'], md['title'][:28],
              corr([a for a, b in pr], [b for a, b in pr]), 12 * slope([a for a, b in pr], [b for a, b in pr]), len(pr)))
