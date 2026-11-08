import sqlite3, sys, math, statistics as st
from bisect import bisect_right
from collections import defaultdict

db = sqlite3.connect(sys.argv[1])
solos = db.execute("""select s.melid,s.performer,s.title,s.style,s.rhythmfeel,s.avgtempo,
  t.lineup,r.artist,r.recordtitle from solo_info s join track_info t using(trackid)
  join record_info r on r.recordid=s.recordid""").fetchall()
beats = defaultdict(list)
for m, o, b, bt in db.execute("select melid,onset,bar,beat from beats order by melid,onset"):
    beats[m].append((o, b, bt))
notes = defaultdict(list)
for m, o, div, tat in db.execute("select melid,onset,division,tatum from melody order by melid,onset"):
    notes[m].append((o, div, tat))

def linfit(xs, ys):
    mx, my = st.fmean(xs), st.fmean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx if sxx else 0
    return my - b * mx, b

def acf(xs, lag):
    if len(xs) <= lag + 2: return None
    m = st.fmean(xs); v = sum((x - m) ** 2 for x in xs)
    if v == 0: return None
    return sum((xs[i] - m) * (xs[i + lag] - m) for i in range(len(xs) - lag)) / v

res = {}
for melid, perf, title, style, feel, avgt, lineup, artist, rtitle in solos:
    B = beats[melid]
    if len(B) < 20: continue
    on = [b[0] for b in B]
    # downbeat onset per bar, and beat-count between downbeats
    down = {}
    idx = {}
    for i, (o, bar, bt) in enumerate(B):
        if bt == 1 and bar not in down: down[bar] = o; idx[bar] = i
    def tempo(b0, b1):
        if b0 in down and b1 in down and down[b1] > down[b0]:
            return 60 * (idx[b1] - idx[b0]) / (down[b1] - down[b0])
    bars = sorted(b for b in down if b >= 1)
    if not bars: continue
    first = bars[0]
    wins = []
    b = first
    while b + 4 in down:
        t = tempo(b, b + 4)
        if t: wins.append(((down[b] + down[b + 4]) / 2, t))
        b += 4
    r = dict(melid=melid, perf=perf, title=title, style=style, feel=feel, lineup=lineup or "",
             artist=artist, rtitle=rtitle)
    allt = tempo(first, max(bars)) if max(bars) > first else None
    r['mean_tempo'] = allt or avgt
    if len(wins) >= 3:
        xs = [w[0] for w in wins]; ys = [w[1] for w in wins]
        a, sl = linfit(xs, ys)
        m = st.fmean(ys)
        resid = [y - (a + sl * x) for x, y in zip(xs, ys)]
        r['win_sd_pct'] = 100 * math.sqrt(sum(e * e for e in resid) / (len(resid) - 2 if len(resid) > 2 else 1)) / m
        r['trend_pct'] = 100 * sl * (xs[-1] - xs[0]) / m
        r['nwin'] = len(wins)
    # 8-bar line: 2 bars before vs 2 after
    ph = []
    for bb in bars:
        if bb > 1 and (bb - 1) % 8 == 0:
            tb, ta = tempo(bb - 2, bb), tempo(bb, bb + 2)
            if tb and ta: ph.append(100 * (ta - tb) / tb)
    r['line8'] = ph
    # placement
    onb, off, offm, sw, swm, seq = [], [], [], [], [], []
    for o, div, tat in notes[melid]:
        i = bisect_right(on, o) - 1
        if i < 0 or i + 1 >= len(on): continue
        d = on[i + 1] - on[i]
        if d <= 0 or d > 2.5: continue
        p = (o - on[i]) / d
        if p < 0.12:
            onb.append(1000 * (o - on[i])); seq.append((i, onb[-1]))
        elif p > 0.88:
            onb.append(1000 * (o - on[i + 1])); seq.append((i + 1, onb[-1]))
        elif 0.45 <= p <= 0.85:
            off.append(1000 * (o - (on[i] + 2 * d / 3)))
            sw.append(p / (1 - p))
            if div == 2 and tat == 2:
                offm.append(off[-1]); swm.append(sw[-1])
    r.update(onb=onb, off=off, sw=sw, offm=offm, swm=swm)
    devs = [v for _, v in seq]
    r['ac'] = {L: acf(devs, L) for L in (1, 2, 4, 8)}
    # adjacent-beat lag-1: pairs of on-beat notes on consecutive beats
    pr = [(seq[k][1], seq[k + 1][1]) for k in range(len(seq) - 1) if seq[k + 1][0] - seq[k][0] == 1]
    r['adjpairs'] = pr
    if len(devs) >= 16:
        ma = [st.fmean(devs[k:k + 8]) for k in range(len(devs) - 7)]
        r['ma8_sd'] = st.pstdev(ma); r['raw_sd'] = st.pstdev(devs)
    res[melid] = r

def tbin(t):
    if t is None: return None
    return "<140" if t < 140 else "140-200" if t < 200 else "200-260" if t < 260 else ">260"

def pearson(pairs):
    if len(pairs) < 10: return None
    x = [a for a, _ in pairs]; y = [b for _, b in pairs]
    mx, my = st.fmean(x), st.fmean(y)
    sx = math.sqrt(sum((a - mx) ** 2 for a in x)); sy = math.sqrt(sum((b - my) ** 2 for b in y))
    return sum((a - mx) * (b - my) for a, b in pairs) / (sx * sy) if sx and sy else None

def f(v, d=1):
    return "-" if v is None else f"{v:.{d}f}"

def summarize(rs):
    out = {}
    out['n'] = len(rs)
    ws = [r for r in rs if 'win_sd_pct' in r]
    out['ntempo'] = len(ws)
    out['tempo'] = st.fmean([r['mean_tempo'] for r in rs if r['mean_tempo']]) if rs else None
    out['winsd'] = st.median([r['win_sd_pct'] for r in ws]) if ws else None
    out['winsd_mean'] = st.fmean([r['win_sd_pct'] for r in ws]) if ws else None
    out['trend'] = st.fmean([r['trend_pct'] for r in ws]) if ws else None
    out['trend_med'] = st.median([r['trend_pct'] for r in ws]) if ws else None
    out['up'] = sum(r['trend_pct'] > 0 for r in ws); out['down'] = sum(r['trend_pct'] < 0 for r in ws)
    out['up1'] = sum(r['trend_pct'] > 1 for r in ws); out['down1'] = sum(r['trend_pct'] < -1 for r in ws)
    l8 = [v for r in rs for v in r['line8']]
    out['l8'] = st.fmean(l8) if l8 else None; out['l8sd'] = st.pstdev(l8) if len(l8) > 1 else None; out['nl8'] = len(l8)
    for k in ('onb', 'off', 'offm'):
        v = [x for r in rs for x in r[k]]
        out[k] = (st.fmean(v), st.pstdev(v), len(v)) if len(v) > 1 else (None, None, len(v))
        per = [st.pstdev(r[k]) for r in rs if len(r[k]) > 5]
        out[k + '_wsd'] = st.fmean(per) if per else None
        pm = [st.fmean(r[k]) for r in rs if len(r[k]) > 5]
        out[k + '_bsd'] = st.pstdev(pm) if len(pm) > 1 else None
    for k in ('sw', 'swm'):
        v = [x for r in rs for x in r[k]]
        out[k] = st.median(v) if v else None
    for L in (1, 2, 4, 8):
        a = [(r['ac'][L], len(r['onb'])) for r in rs if r['ac'][L] is not None]
        out[f'ac{L}'] = sum(x * w for x, w in a) / sum(w for _, w in a) if a else None
    adj = [pearson(r['adjpairs']) for r in rs]
    adjw = [(p, len(r['adjpairs'])) for p, r in zip(adj, rs) if p is not None]
    out['acadj'] = sum(x * w for x, w in adjw) / sum(w for _, w in adjw) if adjw else None
    out['nadj'] = sum(len(r['adjpairs']) for r in rs)
    m = [r for r in rs if 'ma8_sd' in r]
    out['ma8'] = st.fmean([r['ma8_sd'] for r in m]) if m else None
    out['rawsd'] = st.fmean([r['raw_sd'] for r in m]) if m else None
    out['ma8ratio'] = st.fmean([r['ma8_sd'] / r['raw_sd'] for r in m if r['raw_sd']]) if m else None
    return out

R = list(res.values())
groups = {}
for p in ['Miles Davis', 'John Coltrane', 'Wayne Shorter', 'Herbie Hancock', 'Sonny Rollins', 'Charlie Parker']:
    groups[p] = [r for r in R if r['perf'] == p]
groups['Hancock in band (any)'] = [r for r in R if 'Hancock' in r['lineup'] or r['perf'] == 'Herbie Hancock']
mdq2 = lambda r: all(x in r['lineup'] for x in ('Hancock', 'Carter', 'Williams')) and ('Miles' in r['lineup'])
groups['Miles 2nd Quintet (all soloists)'] = [r for r in R if mdq2(r)]
mdq1 = lambda r: all(x in r['lineup'] for x in ('Garland', 'Chambers', 'Philly Joe')) and 'Miles' in r['lineup']
groups['Miles 1st Quintet (all soloists)'] = [r for r in R if mdq1(r)]
jcq = lambda r: 'Tyner' in r['lineup'] and 'Elvin' in r['lineup'] and r['perf'] == 'John Coltrane'
groups['Coltrane Quartet (Tyner+Elvin)'] = [r for r in R if jcq(r)]
groups['ALL'] = R
styles = {s: [r for r in R if r['style'] == s] for s in sorted({r['style'] for r in R})}
feels = {s: [r for r in R if r['feel'] == s] for s in sorted({r['feel'] for r in R})}
bins = ["<140", "140-200", "200-260", ">260"]

def table(title, gs):
    S = {k: summarize(v) for k, v in gs.items() if v}
    print(f"\n=== {title}: TEMPO ===")
    print(f"{'group':34} {'solos':>5} {'n4w':>4} {'bpm':>6} {'4barSD%med':>10} {'(mean)':>6} {'trend%':>7} {'(med)':>6} {'up/dn':>7} {'>1%up/dn':>9} {'8bar:aft-bef%':>13} {'sd':>5} {'n':>5}")
    for k, s in S.items():
        print(f"{k:34} {s['n']:5} {s['ntempo']:4} {f(s['tempo']):>6} {f(s['winsd'],2):>10} {f(s['winsd_mean'],2):>6} {f(s['trend'],2):>7} {f(s['trend_med'],2):>6} {str(s['up'])+'/'+str(s['down']):>7} {str(s['up1'])+'/'+str(s['down1']):>9} {f(s['l8'],2):>13} {f(s['l8sd'],2):>5} {s['nl8']:5}")
    print(f"\n=== {title}: PLACEMENT (ms, + = behind beat) ===")
    print(f"{'group':34} {'onbeat mean':>11} {'sd':>5} {'wSD':>5} {'bSD':>5} {'n':>6} | {'off(2/3) mean':>13} {'sd':>5} {'wSD':>5} {'n':>6} {'swing med':>9} | {'metr8th off':>11} {'sd':>5} {'n':>6} {'swing':>6}")
    for k, s in S.items():
        o, of, om = s['onb'], s['off'], s['offm']
        print(f"{k:34} {f(o[0]):>11} {f(o[1]):>5} {f(s['onb_wsd']):>5} {f(s['onb_bsd']):>5} {o[2]:6} | {f(of[0]):>13} {f(of[1]):>5} {f(s['off_wsd']):>5} {of[2]:6} {f(s['sw'],2):>9} | {f(om[0]):>11} {f(om[1]):>5} {om[2]:6} {f(s['swm'],2):>6}")
    print(f"\n=== {title}: ON-BEAT DEVIATION STRUCTURE ===")
    print(f"{'group':34} {'ac1':>5} {'ac2':>5} {'ac4':>5} {'ac8':>5} {'adjBeat r':>9} {'pairs':>6} {'rawSD':>6} {'MA8 SD':>6} {'ratio':>5} (white noise ratio = 0.35)")
    for k, s in S.items():
        print(f"{k:34} {f(s['ac1'],2):>5} {f(s['ac2'],2):>5} {f(s['ac4'],2):>5} {f(s['ac8'],2):>5} {f(s['acadj'],2):>9} {s['nadj']:6} {f(s['rawsd']):>6} {f(s['ma8']):>6} {f(s['ma8ratio'],2):>5}")
    return S

table("PERFORMER", groups)
table("STYLE", styles)
table("RHYTHM FEEL", feels)
tb = {}
for g in ['ALL', 'Miles Davis', 'John Coltrane', 'Wayne Shorter', 'Sonny Rollins', 'Charlie Parker', 'Miles 2nd Quintet (all soloists)']:
    for b in bins:
        tb[f"{g} {b}"] = [r for r in groups[g] if tbin(r['mean_tempo']) == b]
for s in ['SWING', 'BEBOP', 'HARDBOP', 'POSTBOP', 'COOL']:
    for b in bins:
        tb[f"{s} {b}"] = [r for r in styles[s] if tbin(r['mean_tempo']) == b]
table("TEMPO BIN", tb)

print("\n=== PER-SOLO (Miles / Coltrane / Shorter / Hancock) ===")
for r in sorted(R, key=lambda r: (r['perf'], r['melid'])):
    if r['perf'] in ('Miles Davis', 'John Coltrane', 'Wayne Shorter', 'Herbie Hancock'):
        tag = 'MDQ2' if mdq2(r) else 'MDQ1' if mdq1(r) else 'JCQ' if jcq(r) else ''
        print(f"{r['melid']:4} {r['perf'][:14]:14} {r['title'][:24]:24} {tag:5} bpm={f(r['mean_tempo'],0):>4} sd%={f(r.get('win_sd_pct'),2):>5} trend%={f(r.get('trend_pct'),2):>6} on={f(st.fmean(r['onb']) if r['onb'] else None):>6}/{f(st.pstdev(r['onb']) if len(r['onb'])>1 else None):>5} off={f(st.fmean(r['off']) if r['off'] else None):>6} sw={f(st.median(r['sw']) if r['sw'] else None,2):>5} ac1={f(r['ac'][1],2)}")
