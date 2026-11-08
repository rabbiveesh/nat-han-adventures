import sqlite3, sys, re, math, statistics as st
from collections import defaultdict

c = sqlite3.connect(sys.argv[1])
meta = {}
for m, perf, style, rd, tempo in c.execute('select s.melid,s.performer,s.style,t.recordingdate,s.avgtempo from solo_info s join track_info t using(trackid)'):
    y = re.findall(r'(19\d\d)', rd or '')
    meta[m] = (perf, style, int(y[0]) if y else 0, tempo)
notes = defaultdict(list)
for r in c.execute('select melid,onset,pitch,duration,division,bar,beat,tatum,beatdur,loud_med,loud_max,loud_relpos,loud_sd,f0_mod from melody order by eventid'):
    notes[r[0]].append(dict(on=r[1], p=r[2], dur=r[3], div=r[4], bar=r[5], beat=r[6], tat=r[7], bd=r[8],
                            lm=r[9] if r[9] and r[9] > 20 else None, lmax=r[10], rp=r[11], lsd=r[12], f0=r[13]))
phr = defaultdict(list)
for m, s, e in c.execute("select melid,start,end from sections where type='PHRASE'"):
    phr[m].append((s, e))

def groups(m):
    p, s, y, t = meta[m]; g = ['ALL', s]
    if p == 'Miles Davis': g.append('Miles 51-59' if y < 1960 else 'Miles 64-69')
    if p == 'John Coltrane': g.append('Coltrane <1960' if y < 1960 else 'Coltrane 1960+')
    for a, b in [('Charlie Parker', 'Parker'), ('Sonny Rollins', 'Rollins'), ('Wayne Shorter', 'Shorter'), ('Louis Armstrong', 'Armstrong'),
                 ('Clifford Brown', 'C.Brown'), ('Dizzy Gillespie', 'Gillespie'), ('Michael Brecker', 'Brecker')]:
        if p == a: g.append(b)
    g.append('tempo<140' if t < 140 else 'tempo140-220' if t < 220 else 'tempo>=220')
    return g
GO = ['Miles 51-59', 'Miles 64-69', 'Coltrane <1960', 'Coltrane 1960+', 'Parker', 'Rollins', 'Shorter', 'C.Brown', 'Gillespie', 'Brecker', 'Armstrong',
      'ALL', 'TRADITIONAL', 'SWING', 'BEBOP', 'COOL', 'HARDBOP', 'POSTBOP', 'tempo<140', 'tempo140-220', 'tempo>=220']
A = defaultdict(lambda: defaultdict(list))
def add(m, k, v):
    for g in groups(m): A[g][k].append(v)
def mean(x): return sum(x) / len(x) if x else float('nan')
def med(x): return st.median(x) if x else float('nan')

for m, ns in notes.items():
    vals = [n['lm'] for n in ns if n['lm'] is not None]
    md = st.median(vals)
    for n in ns: n['rel'] = None if n['lm'] is None else n['lm'] - md
    end_of = {}
    for s, e in phr[m]:
        for i in range(s, e + 1): end_of[i] = e
    # per-note IOI to next note in the same phrase
    for i, n in enumerate(ns):
        if i + 1 < len(ns) and end_of.get(i) == end_of.get(i + 1) and i != end_of.get(i):
            n['ioi'] = ns[i + 1]['on'] - n['on']; n['ioib'] = n['ioi'] / n['bd']
        else:
            n['ioi'] = None
    # runs: maximal stretches where ioi <= 0.55 beat (8ths and faster) inside a phrase, >= 5 notes
    i = 0
    while i < len(ns):
        j = i
        while j < len(ns) and ns[j]['ioi'] is not None and ns[j]['ioib'] <= .55: j += 1
        run = ns[i:j + 1]  # note j ends the run (its ioi not run-ish)
        if len(run) >= 5:
            for k, n in enumerate(run): n['run'] = (k, len(run))
        i = max(j + 1, i + 1)
    # note-level stats
    for i, n in enumerate(ns):
        inrun = 'run' in n
        if n['ioi'] and n['ioi'] > 0:
            ar = n['dur'] / n['ioi']
            sec = n['ioi']
            b = 'ioi<0.10s' if sec < .10 else 'ioi.10-.15' if sec < .15 else 'ioi.15-.20' if sec < .2 else 'ioi.20-.30' if sec < .3 else 'ioi.30-.50' if sec < .5 else 'ioi>.50'
            add(m, 'ar_' + b, ar)
            if n['rel'] is not None: add(m, 'rel_' + b, n['rel'])
            if n['rp'] is not None: add(m, 'rp_' + b, n['rp'])
            if n['lsd'] is not None: add(m, 'lsd_' + b, n['lsd'])
            if inrun and n['run'][0] < n['run'][1] - 1:
                add(m, 'ar_run', ar); add(m, 'leg_run', ar >= .9); add(m, 'det_run', ar < .6)
            elif not inrun:
                add(m, 'ar_non', ar); add(m, 'leg_non', ar >= .9); add(m, 'det_non', ar < .6)
            if inrun: add(m, 'slide_run', n['f0'] == 'slide')
    # run-level stats
    runs = []
    cur = []
    for n in ns:
        if 'run' in n:
            if n['run'][0] == 0 and cur: runs.append(cur); cur = []
            cur.append(n)
    if cur: runs.append(cur)
    for run in runs:
        r = [n['rel'] for n in run]
        if any(x is None for x in r): continue
        rm = mean(r); L = len(run)
        add(m, 'run_len', L)
        add(m, 'run_mean', rm)
        add(m, 'run_sd', st.pstdev(r))
        add(m, 'run_range', max(r) - min(r))
        add(m, 'run_tempo_ioi', med([n['ioi'] for n in run[:-1]]))
        for k, n in enumerate(run):
            d = n['rel'] - rm
            pos = 'first' if k == 0 else 'last' if k == L - 1 else 'second' if k == 1 else 'penult' if k == L - 2 else 'inner'
            add(m, 'pos_' + pos, d)
            add(m, 'ghost', d < -3); add(m, 'accent', d > 3)
            # metric inside run
            if n['tat'] == 1: add(m, 'met_on', d)
            elif n['div'] == 2 and n['tat'] == 2: add(m, 'met_and8', d)
            elif n['div'] in (3, 6): add(m, 'met_trip', d)
            elif n['div'] == 4: add(m, 'met_16_%d' % n['tat'], d)
            else: add(m, 'met_other', d)
        # direction changes / peaks
        mx = max(n['p'] for n in run); mn = min(n['p'] for n in run)
        hi = [n['rel'] - rm for n in run if n['p'] == mx]; lo = [n['rel'] - rm for n in run if n['p'] == mn]
        add(m, 'run_top', mean(hi)); add(m, 'run_bottom', mean(lo))
        # turnaround notes (local pitch extremum inside run)
        for k in range(1, L - 1):
            a, b, cc = run[k - 1]['p'], run[k]['p'], run[k + 1]['p']
            if (b > a and b > cc) or (b < a and b < cc): add(m, 'turn', run[k]['rel'] - rm)
            else: add(m, 'pass', run[k]['rel'] - rm)
        # chromatic/step pitch-loudness inside run
        pm = mean([n['p'] for n in run])
        for n in run: add(m, 'pl', (n['p'] - pm, n['rel'] - rm))
        # rp (attack proxy) run notes vs the run's first note
        add(m, 'rp_runfirst', run[0]['rp']); 
        for n in run[1:-1]: add(m, 'rp_runinner', n['rp'])
        add(m, 'lmaxmed_inner', mean([n['lmax'] for n in run[1:-1] if n['lmax'] is not None]) if L > 2 else None)
    # share of notes in runs
    for n in ns: add(m, 'in_run', 'run' in n)

def slope(pr):
    xs = [a for a, b in pr]; ys = [b for a, b in pr]; mx, my = mean(xs), mean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    syy = sum((y - my) ** 2 for y in ys)
    return 12 * sxy / sxx, sxy / math.sqrt(sxx * syy)

def T(title, cols, fmt='%+.2f'):
    print('\n' + title)
    print('[' + ' | '.join(h for h, _ in cols) + ']')
    for g in GO:
        if g not in A: continue
        out = []
        nset = set()
        for h, f in cols:
            v, n = f(g); nset.add(n)
            out.append((('%s' % '-') if v != v else fmt % v, n))
        if len(nset) == 1: print('%s %s n=%d' % (g, ' '.join(a for a, b in out), nset.pop()))
        else: print('%s %s' % (g, ' '.join('%s(%d)' % o for o in out)))
M = lambda k, fn=mean: (lambda g: (fn([x for x in A[g][k] if x is not None]), len([x for x in A[g][k] if x is not None])))
SH = lambda k: (lambda g: (100 * mean([1 if x else 0 for x in A[g][k]]), len(A[g][k])))

T('R0 runs = 5+ notes inside a phrase, each IOI <= 0.55 beat. [% of notes in runs | mean run length | median IOI in run, s]',
  [('% notes', SH('in_run')), ('len', M('run_len')), ('ioi s', M('run_tempo_ioi', med))], '%.2f')
B = ['ioi<0.10s', 'ioi.10-.15', 'ioi.15-.20', 'ioi.20-.30', 'ioi.30-.50', 'ioi>.50']
T('R1 articulation ratio = duration / IOI to the next note (1 = no gap, legato/slur; <0.6 = detached), median, by IOI in seconds',
  [(b, M('ar_' + b, med)) for b in B], '%.2f')
T('R2 loudness (dB vs solo median), by IOI in seconds', [(b, M('rel_' + b)) for b in B])
T('R3 attack proxy loud_relpos (where the note\'s loudness peak falls, 0 = at the onset), by IOI in seconds', [(b, M('rp_' + b)) for b in B], '%.2f')
T('R3b within-note loudness SD (dB), by IOI in seconds', [(b, M('lsd_' + b)) for b in B], '%.2f')
T('R4 articulation in runs vs outside: [median ratio run | % legato (>=0.9) run | % detached (<0.6) run | median ratio non-run | % legato non-run | % detached non-run]',
  [('ar run', M('ar_run', med)), ('leg run', SH('leg_run')), ('det run', SH('det_run')), ('ar non', M('ar_non', med)), ('leg non', SH('leg_non')), ('det non', SH('det_non'))], '%.2f')
T('R5 run loudness: [run mean vs solo median | SD inside the run | max-min inside run, dB] n = runs',
  [('mean', M('run_mean')), ('sd', M('run_sd')), ('range', M('run_range'))])
T('R6 position in the run, dB vs the run mean [first | second | inner | penultimate | last]',
  [(p, M('pos_' + p)) for p in ('first', 'second', 'inner', 'penult', 'last')])
T('R7 metric position inside runs, dB vs run mean [on-beat | swung "and" (div 2 tat 2) | triplet offbeats | 16ths: e | and | a]',
  [('on', M('met_on')), ('and8', M('met_and8')), ('trip', M('met_trip')), ('16e', M('met_16_2')), ('16and', M('met_16_3')), ('16a', M('met_16_4'))])
T('R8 shape inside runs: [top note | bottom note | turnaround (local extreme) | passing note, dB vs run mean | % ghost (< -3 dB) | % accent (> +3 dB)]',
  [('top', M('run_top')), ('bottom', M('run_bottom')), ('turn', M('turn')), ('pass', M('pass')), ('ghost%', SH('ghost')), ('accent%', SH('accent'))])
print('\nR9 pitch vs loudness inside runs (run-centred): [dB/octave | r]')
for g in GO:
    if g in A and len(A[g]['pl']) > 50:
        s, r = slope(A[g]['pl']); print('%s %+.2f %+.2f n=%d' % (g, s, r, len(A[g]['pl'])))
T('R10 attack proxy in runs [loud_relpos first note | inner notes] and % of run notes with a slide', [('rp first', M('rp_runfirst')), ('rp inner', M('rp_runinner')), ('slide%', SH('slide_run'))], '%.3f')
