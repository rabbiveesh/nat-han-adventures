//! The band's thinking: what each musician decided, bar by bar, scrolling under the playhead.
//! The soloist's plan on top (the chorus's shape, each phrase: what it's about, where it aims to
//! land, the skeleton of target notes it steers at, what it heard and did about it), then the
//! band's shared plan (chorus, feel, hits, fills, reharms) and what the comp, bass and drums
//! played beyond the written part.

use bevy_egui::egui::{self, Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use nat_han_adventures::audio::live::band::{Fill, SubKind, Trade};
use nat_han_adventures::audio::live::engine::CommittedBar;
use nat_han_adventures::audio::live::feel::Feel;
use nat_han_adventures::audio::live::ornament::Orn;
use nat_han_adventures::audio::live::soloist::{
    self, HEARD_CRASH, HEARD_FILL, HEARD_HIT_END, HEARD_HIT_IN, Idea, Soloist,
};

use crate::app::Editor;
use crate::theme::*;

/// Bars on screen, and how far into them the playhead sits.
const SHOWN: f64 = 6.0;
const AHEAD: f64 = 1.6;
const LABEL_W: f32 = 92.0;

/// Each kind of phrase's colour.
fn idea_color(i: Idea) -> Color32 {
    match i {
        Idea::New => CHANNEL[0],
        Idea::Restate => GOLD,
        Idea::Fragment => Color32::from_rgb(0xff, 0xd2, 0x6b),
        Idea::Lick => CHANNEL[2],
        Idea::Echo => CHANNEL[1],
        Idea::Carry => GREEN,
    }
}

pub fn ui(ed: &mut Editor, ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, BG);
    if !ed.player.playing || ed.thinking.is_empty() {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Play to see the band think (space).",
            mono(14.0),
            TEXT_FAINT,
        );
        return;
    }
    let chart = ed.played().played_chart.clone();
    let heard = ed.player.heard(ed.now);
    let bb = heard.beats_per_bar.max(1.0);
    let head = heard.position.bar as f64 + (heard.position.beat / bb).clamp(0.0, 0.999);
    let (t0, t1) = (head - (SHOWN - AHEAD), head + AHEAD);
    let grid = Rect::from_min_max(pos2(rect.left() + LABEL_W, rect.top()), rect.max);
    // Absolute bars (fractional) to x.
    let x_of = |bar: f64| grid.left() + ((bar - t0) / (t1 - t0)) as f32 * grid.width();
    let bars: Vec<&CommittedBar> = ed
        .thinking
        .range((t0.floor() as u64).saturating_sub(8)..=(t1.ceil() as u64))
        .map(|(_, b)| b)
        .collect();
    let font = mono(12.0);
    let small = mono(10.5);

    // Lanes, top to bottom.
    let mut y = rect.top() + 4.0;
    let lane = |y: &mut f32, h: f32| {
        let r = Rect::from_min_size(pos2(rect.left(), *y), vec2(rect.width(), h));
        *y += h + 6.0;
        r
    };
    let head_r = lane(&mut y, 24.0);
    let band_r = lane(&mut y, 44.0);
    let arc_r = lane(&mut y, 54.0);
    let phrase_r = lane(&mut y, 40.0);
    let roll_h = (rect.bottom() - y - 3.0 * 36.0 - 3.0 * 6.0 - 30.0).max(160.0);
    let roll_r = lane(&mut y, roll_h);
    let heard_r = lane(&mut y, 22.0);
    let comp_r = lane(&mut y, 30.0);
    let bass_r = lane(&mut y, 30.0);
    let drums_r = lane(&mut y, 30.0);
    let caption = |r: Rect, s: &str, c: Color32| {
        p.text(
            pos2(r.left() + 6.0, r.center().y),
            Align2::LEFT_CENTER,
            s,
            mono(11.0),
            c,
        );
    };
    for (r, s, c) in [
        (band_r, "BAND", TEXT_DIM),
        (arc_r, "SOLO ARC", CHANNEL[0]),
        (phrase_r, "PHRASES", CHANNEL[0]),
        (roll_r, "LINE", CHANNEL[0]),
        (heard_r, "HEARD", TEXT_DIM),
        (comp_r, "COMP", CHANNEL[1]),
        (bass_r, "BASS", CHANNEL[2]),
        (drums_r, "DRUMS", CHANNEL[3]),
    ] {
        p.rect_filled(
            Rect::from_min_max(pos2(grid.left(), r.top()), r.max),
            3.0,
            PANEL,
        );
        caption(r, s, c);
    }

    // Bar lines, bar numbers and chords.
    for bar in t0.floor() as i64..=t1.ceil() as i64 {
        if bar < 0 {
            continue;
        }
        let x = x_of(bar as f64);
        if x < grid.left() - 1.0 || x > grid.right() + 1.0 {
            continue;
        }
        p.line_segment(
            [pos2(x, head_r.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, BAR_LINE),
        );
        for k in 1..bb.round() as i64 {
            let xb = x_of(bar as f64 + k as f64 / bb);
            if xb > grid.left() && xb < grid.right() {
                p.line_segment(
                    [pos2(xb, arc_r.top()), pos2(xb, drums_r.bottom())],
                    Stroke::new(1.0, LINE_SOFT),
                );
            }
        }
        if let Some(c) = ed.thinking.get(&(bar as u64)) {
            let song_bar = c.slot.song_bar + ed.player.offset;
            let chord = chart[..=song_bar.min(chart.len().saturating_sub(1))]
                .iter()
                .rev()
                .find(|s| *s != "%")
                .cloned()
                .unwrap_or_default();
            let clip = Rect::from_min_max(
                pos2(x.max(grid.left()), head_r.top()),
                pos2(x_of(bar as f64 + 1.0).min(grid.right()), head_r.bottom()),
            );
            let pa = ui.painter_at(clip);
            pa.text(
                pos2(x + 5.0, head_r.center().y),
                Align2::LEFT_CENTER,
                format!("{}", song_bar + 1),
                small.clone(),
                TEXT_FAINT,
            );
            pa.text(
                pos2(x + 32.0, head_r.center().y),
                Align2::LEFT_CENTER,
                chord,
                font.clone(),
                TEXT,
            );
        }
    }

    let clip_x = |a: f32, b: f32| (a.max(grid.left()), b.min(grid.right()));

    // BAND: chorus and feel at their changes, hits, fills, crashes, trades, reharms, soloist.
    let mut prev: Option<&CommittedBar> = None;
    for c in &bars {
        let b0 = c.slot.index as f64;
        let (xa, xb) = (x_of(b0), x_of(b0 + 1.0));
        if xb < grid.left() || xa > grid.right() {
            prev = Some(c);
            continue;
        }
        let pa = ui.painter_at(Rect::from_min_max(
            pos2(xa.max(grid.left()), band_r.top()),
            pos2(xb.min(grid.right()), band_r.bottom()),
        ));
        let first = xa <= grid.left() + 1.0;
        let changed = prev.is_none_or(|q| {
            q.band.chorus != c.band.chorus || q.band.feel != c.band.feel || q.soloist != c.soloist
        });
        if changed || first {
            let chorus = match c.band.chorus.label() {
                "" => "HEAD",
                s => s,
            };
            let feel = if c.band.feel == Feel::Swing {
                String::new()
            } else {
                format!(" · {}", c.band.feel.label().to_lowercase())
            };
            let who = if c.soloist == Soloist::Band {
                String::new()
            } else {
                format!(" · {}", soloist_name(c.soloist))
            };
            let at = xa.max(grid.left());
            ui.painter_at(Rect::from_min_max(
                pos2(at, band_r.top()),
                pos2(grid.right(), band_r.bottom()),
            ))
            .text(
                pos2(at + 5.0, band_r.top() + 10.0),
                Align2::LEFT_CENTER,
                format!("{chorus}{feel}{who}"),
                font.clone(),
                TEXT,
            );
        }
        let mut notes: Vec<String> = Vec::new();
        for s in c.band.subs.iter().flatten() {
            let kind = match s.kind {
                SubKind::Tritone => "tritone sub",
                SubKind::Backdoor => "backdoor",
                SubKind::IiV => "ii",
                SubKind::Arranged => "arranged",
            };
            notes.push(format!("{} {kind}", s.chord));
        }
        match c.band.trade {
            Trade::Lead => notes.push("lead's 4".into()),
            Trade::Drums => notes.push("drums' 4".into()),
            Trade::None => {}
        }
        if c.band.solo_break {
            notes.push("break".into());
        }
        if c.band.tacet {
            notes.push("tacet".into());
        }
        if !notes.is_empty() {
            pa.text(
                pos2(xa + 5.0, band_r.bottom() - 9.0),
                Align2::LEFT_CENTER,
                notes.join(" · "),
                small.clone(),
                GOLD,
            );
        }
        // Hits: gold ticks on their 16ths.
        for k in 0..16 {
            if c.band.hits & (1 << k) != 0 {
                let x = x_of(b0 + k as f64 / 16.0);
                pa.rect_filled(
                    Rect::from_center_size(pos2(x, band_r.center().y + 4.0), vec2(4.0, 14.0)),
                    1.0,
                    GOLD,
                );
            }
        }
        let fill = match c.band.fill {
            Fill::None => None,
            Fill::Short => Some(1.0),
            Fill::Full | Fill::PressRoll => Some(2.0),
        };
        if let Some(beats) = fill {
            let (fa, fb) = (x_of(b0 + 1.0 - beats / bb), xb);
            let r = Rect::from_min_max(
                pos2(fa, band_r.center().y - 2.0),
                pos2(fb - 2.0, band_r.center().y + 10.0),
            );
            pa.rect_filled(r, 2.0, CHANNEL[3].gamma_multiply(0.35));
            pa.text(
                pos2(r.left() + 4.0, r.center().y),
                Align2::LEFT_CENTER,
                "fill",
                small.clone(),
                CHANNEL[3],
            );
        }
        if c.band.crash {
            star(
                &pa,
                pos2(xa + 4.0, band_r.center().y + 4.0),
                6.0,
                CHANNEL[3],
            );
        }
        prev = Some(c);
    }

    // SOLO ARC: the heat curve, the shape named at each chorus, the 16ths' weight.
    let mut pts: Vec<Pos2> = Vec::new();
    let mut last_arc = None;
    for c in &bars {
        let Some(t) = c.thought else {
            last_arc = None;
            continue;
        };
        let Some(h) = t.heat else { continue };
        let b0 = c.slot.index as f64;
        let x = x_of(b0 + 0.5);
        let yh = arc_r.bottom() - 6.0 - h * (arc_r.height() - 16.0);
        pts.push(pos2(x, yh));
        let first = x_of(b0) <= grid.left() + 1.0 && x_of(b0 + 1.0) > grid.left();
        if (last_arc != Some(t.arc) || first) && x_of(b0 + 1.0) > grid.left() && x < grid.right() {
            let xa = x_of(b0).max(grid.left());
            ui.painter_at(Rect::from_min_max(pos2(xa, arc_r.top()), arc_r.max))
                .text(
                    pos2(xa + 4.0, arc_r.top() + 8.0),
                    Align2::LEFT_CENTER,
                    format!("{} · 16ths weight ×{:.2}", t.arc.label(), t.dense),
                    small.clone(),
                    CHANNEL[0],
                );
        }
        last_arc = Some(t.arc);
    }
    if pts.len() >= 2 {
        let ap = ui.painter_at(Rect::from_min_max(
            pos2(grid.left(), arc_r.top()),
            arc_r.max,
        ));
        ap.add(Shape::line(pts.clone(), Stroke::new(2.0, CHANNEL[0])));
        for q in &pts {
            ap.circle_filled(*q, 2.5, CHANNEL[0]);
        }
    }

    // The lead's notes, for the roll's range.
    let mut notes: Vec<(f64, f64, u8)> = Vec::new();
    for c in &bars {
        if let Some(t) = c.thought {
            let b0 = c.slot.index as f64;
            for &(at, len, n) in &t.notes[..t.nnotes as usize] {
                notes.push((b0 + at as f64 / bb, len as f64 / bb, n));
            }
        }
    }
    let visible = |a: f64| a > t0 - 0.5 && a < t1 + 0.5;
    let (mut lo, mut hi) = (127u8, 0u8);
    for &(_, _, n) in notes.iter().filter(|x| visible(x.0)) {
        (lo, hi) = (lo.min(n), hi.max(n));
    }
    for c in &bars {
        if let Some(t) = c.thought {
            for ph in &t.phrases[..t.nphrases as usize] {
                for &(_, g) in &ph.goals[..ph.ngoals as usize] {
                    (lo, hi) = (lo.min(g), hi.max(g));
                }
            }
        }
    }
    if lo > hi {
        (lo, hi) = (60, 72);
    }
    let (lo, hi) = (lo.saturating_sub(2) as f32, (hi + 2) as f32);
    let y_of =
        |n: f32| roll_r.bottom() - 4.0 - (n - lo) / (hi - lo).max(1.0) * (roll_r.height() - 8.0);
    let row_h = ((roll_r.height() - 8.0) / (hi - lo).max(1.0)).clamp(3.0, 12.0);
    let rp = ui.painter_at(Rect::from_min_max(
        pos2(grid.left(), roll_r.top()),
        roll_r.max,
    ));
    // Octave lines (C).
    for n in (lo.ceil() as i32..=hi as i32).filter(|n| n % 12 == 0) {
        let yy = y_of(n as f32);
        rp.line_segment(
            [pos2(grid.left(), yy), pos2(grid.right(), yy)],
            Stroke::new(1.0, LINE),
        );
        p.text(
            pos2(grid.left() - 6.0, yy),
            Align2::RIGHT_CENTER,
            format!("C{}", n / 12 - 1),
            mono(9.5),
            TEXT_FAINT,
        );
    }
    for &(a, len, n) in &notes {
        let (xa, xb) = (x_of(a), x_of(a + len));
        if xb < grid.left() || xa > grid.right() {
            continue;
        }
        let yy = y_of(n as f32);
        let past = a < head;
        let col = if past {
            CHANNEL[0]
        } else {
            CHANNEL[0].gamma_multiply(0.45)
        };
        rp.rect_filled(
            Rect::from_min_max(
                pos2(xa, yy - row_h * 0.5),
                pos2(xb.max(xa + 3.0), yy + row_h * 0.5),
            ),
            2.0,
            col,
        );
    }

    // PHRASES: each phrase's span, what it's about, where it aims to land; its skeleton on the
    // roll (diamonds: filled when the line landed on it).
    let pp = ui.painter_at(Rect::from_min_max(
        pos2(grid.left(), phrase_r.top()),
        phrase_r.max,
    ));
    let mut tips: Vec<(Rect, String)> = Vec::new();
    for c in &bars {
        let Some(t) = c.thought else { continue };
        let b0 = c.slot.index as f64;
        for ph in &t.phrases[..t.nphrases as usize] {
            let a = b0 + ph.at as f64 / bb;
            let z = b0 + ph.end as f64 / bb;
            let (xa, xb) = (x_of(a), x_of(z));
            if xb < grid.left() || xa > grid.right() {
                continue;
            }
            let col = idea_color(ph.idea);
            let r = Rect::from_min_max(
                pos2(xa + 1.0, phrase_r.top() + 3.0),
                pos2(xb - 1.0, phrase_r.top() + 21.0),
            );
            pp.rect_filled(r, 4.0, col.gamma_multiply(0.28));
            pp.rect_stroke(r, 4.0, Stroke::new(1.0, col), egui::StrokeKind::Inside);
            let label = match ph.idea {
                Idea::Restate | Idea::Fragment if ph.by != 0 => {
                    format!("{} {:+}", ph.idea.label(), ph.by)
                }
                _ => ph.idea.label().to_string(),
            };
            let (cx0, cx1) = clip_x(r.left(), r.right());
            ui.painter_at(Rect::from_min_max(
                pos2(cx0, r.top()),
                pos2(cx1, r.bottom()),
            ))
            .text(
                pos2(cx0 + 6.0, r.center().y),
                Align2::LEFT_CENTER,
                label,
                small.clone(),
                TEXT,
            );
            // The aimed landing: a flag, "ant." when it's anticipated an 8th early.
            if xb > grid.left() && xb < grid.right() {
                pp.line_segment(
                    [pos2(xb, phrase_r.top() + 2.0), pos2(xb, phrase_r.bottom())],
                    Stroke::new(2.0, col),
                );
                let beat = (ph.end as f64).rem_euclid(bb) + 1.0;
                let txt = if ph.push {
                    format!("→ {beat:.1} ant.")
                } else {
                    format!("→ {beat:.1}")
                };
                pp.text(
                    pos2(xb - 3.0, phrase_r.bottom() - 7.0),
                    Align2::RIGHT_CENTER,
                    txt,
                    mono(9.5),
                    col,
                );
            }
            let mut goals = Vec::new();
            for &(gt, g) in &ph.goals[..ph.ngoals as usize] {
                let ga = b0 + gt as f64 / bb;
                let gx = x_of(ga);
                let gy = y_of(g as f32);
                let hit = notes
                    .iter()
                    .any(|&(na, _, n)| n == g && (na - ga).abs() < 0.3 / bb);
                if gx >= grid.left() && gx <= grid.right() {
                    diamond(&rp, pos2(gx, gy), 6.0, GOLD, hit);
                }
                goals.push(format!("{}", note_name(g)));
            }
            ngoal_line(&rp, ph, b0, bb, &x_of, &y_of);
            tips.push((
                Rect::from_min_max(pos2(cx0, r.top()), pos2(cx1, r.bottom())),
                format!(
                    "{} · {:.1} beats · lands on beat {:.1}{}\nskeleton: {}",
                    ph.idea.label(),
                    ph.len,
                    (ph.end as f64).rem_euclid(bb) + 1.0,
                    if ph.push { " (anticipated)" } else { "" },
                    if goals.is_empty() {
                        "none".into()
                    } else {
                        goals.join(" → ")
                    }
                ),
            ));
        }
        // HEARD: what the soloist heard and acted on.
        let xa = x_of(b0);
        if xa > grid.left() - 1.0 && xa < grid.right() {
            let mut heard = Vec::new();
            for (bit, s) in [
                (HEARD_HIT_END, "lands on the hit"),
                (HEARD_HIT_IN, "in on the hit"),
                (HEARD_CRASH, "in on the crash"),
                (HEARD_FILL, "out for the fill"),
            ] {
                if t.listened & bit != 0 {
                    heard.push(s);
                }
            }
            if !heard.is_empty() {
                let hp = ui.painter_at(Rect::from_min_max(
                    pos2(xa.max(grid.left()), heard_r.top()),
                    pos2(x_of(b0 + 1.0).min(grid.right()), heard_r.bottom()),
                ));
                hp.text(
                    pos2(xa + 5.0, heard_r.center().y),
                    Align2::LEFT_CENTER,
                    format!("👂 {}", heard.join(", ")),
                    small.clone(),
                    GOLD,
                );
            }
        }
    }
    for (r, tip) in tips {
        ui.interact(r, ui.id().with(("phrase", r.left() as i32)), Sense::hover())
            .on_hover_text(tip);
    }

    // COMP, BASS, DRUMS: what each played beyond the written part.
    for (ch, r) in [(1usize, comp_r), (2, bass_r), (3, drums_r)] {
        for c in &bars {
            let b0 = c.slot.index as f64;
            let (xa, xb) = (x_of(b0), x_of(b0 + 1.0));
            if xb < grid.left() || xa > grid.right() {
                continue;
            }
            let names: Vec<&str> = c.orns[ch]
                .iter()
                .filter(|o| !matches!(o, Orn::Walking))
                .map(|o| o.name())
                .collect();
            let walk = c.orns[ch].has(Orn::Walking);
            let mut text = names.join(" · ");
            if walk {
                text = if text.is_empty() {
                    "walking".into()
                } else {
                    format!("walking · {text}")
                };
            }
            if text.is_empty() {
                continue;
            }
            let answer = ch == 1 && c.orns[1].has(Orn::CompAnswer);
            let pa = ui.painter_at(Rect::from_min_max(
                pos2(xa.max(grid.left()) + 2.0, r.top()),
                pos2(xb.min(grid.right()) - 2.0, r.bottom()),
            ));
            if answer {
                pa.rect_filled(
                    Rect::from_min_max(
                        pos2(xa + 2.0, r.top() + 3.0),
                        pos2(xb - 2.0, r.bottom() - 3.0),
                    ),
                    3.0,
                    CHANNEL[1].gamma_multiply(0.25),
                );
            }
            pa.text(
                pos2(xa + 6.0, r.center().y),
                Align2::LEFT_CENTER,
                text,
                small.clone(),
                CHANNEL[ch],
            );
        }
    }

    // The playhead.
    let x = x_of(head);
    p.rect_filled(
        Rect::from_min_max(pos2(x - 3.0, head_r.bottom()), pos2(x + 3.0, rect.bottom())),
        0.0,
        GOLD.gamma_multiply(0.10),
    );
    p.line_segment(
        [pos2(x, head_r.bottom()), pos2(x, rect.bottom())],
        Stroke::new(2.0, GOLD),
    );
    // The key.
    let mut kx = grid.left() + 4.0;
    let ky = rect.bottom() - 10.0;
    for (i, s) in [
        (Idea::New, "new"),
        (Idea::Carry, "carries a cell"),
        (Idea::Restate, "restate"),
        (Idea::Echo, "echoes the comp"),
        (Idea::Lick, "lick"),
    ] {
        p.rect_filled(
            Rect::from_min_size(pos2(kx, ky - 5.0), vec2(10.0, 10.0)),
            2.0,
            idea_color(i),
        );
        let r = p.text(
            pos2(kx + 14.0, ky),
            Align2::LEFT_CENTER,
            s,
            mono(10.0),
            TEXT_DIM,
        );
        kx = r.right() + 14.0;
    }
    diamond(&p, pos2(kx + 5.0, ky), 5.0, GOLD, false);
    let r = p.text(
        pos2(kx + 14.0, ky),
        Align2::LEFT_CENTER,
        "target note (filled: landed)",
        mono(10.0),
        TEXT_DIM,
    );
    kx = r.right() + 14.0;
    p.text(
        pos2(kx, ky),
        Align2::LEFT_CENTER,
        "→ beat: where the phrase aims to end",
        mono(10.0),
        TEXT_DIM,
    );
}

/// The skeleton's path, a faint dashed line through its targets.
fn ngoal_line(
    p: &egui::Painter,
    ph: &soloist::PhraseThought,
    b0: f64,
    bb: f64,
    x_of: &impl Fn(f64) -> f32,
    y_of: &impl Fn(f32) -> f32,
) {
    let pts: Vec<Pos2> = ph.goals[..ph.ngoals as usize]
        .iter()
        .map(|&(gt, g)| pos2(x_of(b0 + gt as f64 / bb), y_of(g as f32)))
        .collect();
    if pts.len() >= 2 {
        p.extend(Shape::dashed_line(
            &pts,
            Stroke::new(1.0, GOLD.gamma_multiply(0.6)),
            4.0,
            4.0,
        ));
    }
}

fn diamond(p: &egui::Painter, c: Pos2, r: f32, col: Color32, filled: bool) {
    let pts = vec![
        pos2(c.x, c.y - r),
        pos2(c.x + r, c.y),
        pos2(c.x, c.y + r),
        pos2(c.x - r, c.y),
    ];
    if filled {
        p.add(Shape::convex_polygon(pts, col, Stroke::new(1.0, BG)));
    } else {
        p.add(Shape::closed_line(pts, Stroke::new(1.5, col)));
    }
}

fn star(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    for k in 0..4 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        let d = vec2(a.cos(), a.sin()) * r;
        p.line_segment([c - d, c + d], Stroke::new(1.5, col));
    }
}

fn note_name(n: u8) -> String {
    const N: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    format!("{}{}", N[(n % 12) as usize], n as i32 / 12 - 1)
}

fn soloist_name(s: Soloist) -> String {
    s.player()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| s.slug().to_string())
}
