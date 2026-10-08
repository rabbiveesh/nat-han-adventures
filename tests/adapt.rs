//! The adaptive difficulty engine (`adapt`): reducer rules, room choice, calibration, story
//! assists, and simulator-level behaviour over hundreds of rooms.

use nat_han_adventures::adapt::{
    AdaptEvent, AssistLevers, BandMood, Cue, FrustrationSignal, Outcome, PlayerProfile, RoomResult, Skill,
    SkillState, StoryAssist, StoryEvent, WindowEntry, band_distribution, band_mood, calibration, next_room, pick_skill, profile, reduce,
    reduce_story, sample_band, sim,
};
use rand::SeedableRng;
use rand::rngs::StdRng;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn start(id: u64, skill: Skill, band: u8, expected: u32) -> AdaptEvent {
    AdaptEvent::RoomStarted { room_id: id, skills: vec![(skill, band)], expected_deaths: expected, par_secs: Some(30.0) }
}

fn finish(deaths: u32) -> AdaptEvent {
    AdaptEvent::RoomFinished(RoomResult {
        deaths,
        time_secs: 30.0,
        toots: 3,
        idle_after_death_secs: 0.0,
        first_death_secs: (deaths > 0).then_some(8.0),
    })
}

/// Play one room of `skill` at `band` with `deaths` deaths (expected 0).
fn room(p: PlayerProfile, skill: Skill, band: u8, deaths: u32) -> PlayerProfile {
    let id = p.rooms_played as u64;
    reduce(reduce(p, start(id, skill, band, 0)), finish(deaths))
}

/// Play `n` rooms at the skill's current center.
fn rooms_at_center(mut p: PlayerProfile, skill: Skill, n: usize, deaths: u32) -> PlayerProfile {
    for _ in 0..n {
        let c = p.center(skill);
        p = room(p, skill, c, deaths);
    }
    p
}

const P: Skill = Skill::Precision;

// ─── Reducer ─────────────────────────────────────────────────────────────────

#[test]
fn reducer_does_not_touch_its_input() {
    let p0 = PlayerProfile::calibrated(4);
    let before = p0.clone();
    let p1 = room(p0.clone(), P, 4, 0);
    assert_eq!(p0, before, "the old profile is unchanged");
    assert_ne!(p1, p0);
    assert_eq!(p1.streak, 1);
    assert_eq!(p0.streak, 0);
    // Same input, same output.
    assert_eq!(room(p0.clone(), P, 4, 0), p1);
}

#[test]
fn promotes_after_four_clean_rooms_at_center() {
    let p = rooms_at_center(PlayerProfile::calibrated(4), P, 3, 0);
    assert_eq!(p.center(P), 4, "3 rooms is not enough evidence");
    let p = rooms_at_center(p, P, 1, 0);
    assert_eq!(p.center(P), 5);
    assert!(p.cues.contains(&Cue::Promoted(P, 5)));
    assert!(p.skill(P).spread < profile::DEFAULT_SPREAD, "promotion narrows the spread");
}

/// A skill centered at `center` whose current-epoch window holds `rooms` as (band, clean).
fn skill_with(center: u8, rooms: &[(u8, bool)]) -> SkillState {
    let mut st = SkillState::new(center);
    for &(band, clean) in rooms {
        let outcome = if clean { Outcome::Clean } else { Outcome::Struggle };
        st.window = st.window.push(WindowEntry { outcome, band, center, epoch: 0, assists: 0.0 });
    }
    st
}

#[test]
fn promotion_and_demotion_thresholds() {
    use profile::{should_demote, should_promote};
    let (c, x) = (true, false);
    // 3 of 4 = 75%: promote. 2 of 3 rooms isn't enough evidence; 5 of 7 (71%) isn't enough.
    assert!(should_promote(&skill_with(4, &[(4, c), (4, c), (4, x), (4, c)])));
    assert!(!should_promote(&skill_with(4, &[(4, c), (4, c), (4, c)])));
    assert!(!should_promote(&skill_with(4, &[(4, c), (4, c), (4, x), (4, c), (4, c), (4, x), (4, c)])));
    // Stretch: ≥2 stretch rooms need ≥60%; a single bad stretch room doesn't block.
    let center4 = [(4, c), (4, c), (4, c), (4, c)];
    let with = |extra: &[(u8, bool)]| skill_with(4, &[&center4[..], extra].concat());
    assert!(should_promote(&with(&[(5, x)])));
    assert!(!should_promote(&with(&[(5, x), (6, c)])), "1/2 = 50% < 60%");
    assert!(should_promote(&with(&[(5, x), (6, c), (5, c)])), "2/3 ≥ 60%");
    // Assisted rooms aren't promotion evidence.
    let mut assisted = skill_with(4, &center4);
    for e in &mut assisted.window.entries {
        e.assists = 0.3;
    }
    assert!(!should_promote(&assisted));
    // Demote below 50% over ≥4; exactly 50% holds; 3 rooms isn't enough.
    assert!(should_demote(&skill_with(4, &[(4, x), (4, c), (4, x), (4, x)])));
    assert!(!should_demote(&skill_with(4, &[(4, x), (4, c), (4, x), (4, c)])));
    assert!(!should_demote(&skill_with(4, &[(4, x), (4, x), (4, x)])));
    // Rooms below the center don't count either way.
    assert!(!should_demote(&skill_with(4, &[(3, x), (3, x), (3, x), (3, x)])));
}

#[test]
fn stretch_rooms_must_be_good_enough_to_promote() {
    // Two failed stretch rooms (band 5) block a promotion from 4.
    let mut p = PlayerProfile::calibrated(4);
    p = room(p, P, 5, 2);
    p = room(p, P, 5, 2);
    p.assists = 0.0;
    let p = rooms_at_center(p, P, 6, 0);
    assert_eq!(p.center(P), 4, "stretch accuracy 0% < 60% blocks promotion");

    // With good stretch rooms it promotes.
    let mut q = PlayerProfile::calibrated(4);
    q = room(q, P, 5, 0);
    q = room(q, P, 5, 0);
    let q = rooms_at_center(q, P, 4, 0);
    assert_eq!(q.center(P), 5);
}

#[test]
fn demotes_below_50_percent_over_four() {
    let mut p = PlayerProfile::calibrated(5);
    for d in [1, 0, 1, 1] {
        p = room(p, P, 5, d);
    }
    assert_eq!(p.center(P), 4, "1/4 clean < 50%");
    assert!(p.cues.contains(&Cue::Demoted(P, 4)));
    assert!(p.skill(P).spread < profile::DEFAULT_SPREAD);

    // Exactly 50% holds.
    let mut q = PlayerProfile::calibrated(5);
    for d in [1, 0, 1, 0] {
        q = room(q, P, 5, d);
    }
    assert_eq!(q.center(P), 5);
}

#[test]
fn old_evidence_does_not_count_after_a_band_change() {
    // Lots of clean stretch rooms at band 5 while centered at 4...
    let mut p = PlayerProfile::calibrated(4);
    for _ in 0..4 {
        p = room(p, P, 5, 0);
    }
    // ...then a promotion to 5.
    let p = rooms_at_center(p, P, 4, 0);
    assert_eq!(p.center(P), 5);
    // The band-5 stretch rooms from before don't count toward promoting out of 5: it needs
    // four fresh rooms at 5.
    let p = rooms_at_center(p, P, 3, 0);
    assert_eq!(p.center(P), 5, "only 3 fresh rooms at the new center");
    let p = rooms_at_center(p, P, 1, 0);
    assert_eq!(p.center(P), 6);
}

#[test]
fn re_promoting_into_a_band_just_fallen_from_needs_more_evidence() {
    let mut p = PlayerProfile::calibrated(5);
    for _ in 0..4 {
        p = room(p, P, 5, 1);
    }
    assert_eq!(p.center(P), 4);
    p.assists = 0.0;
    let p = rooms_at_center(p, P, profile::MIN_EVIDENCE, 0);
    assert_eq!(p.center(P), 4, "hysteresis: 4 rooms isn't enough to go straight back");
    let p = rooms_at_center(p, P, profile::REPROMOTE_EVIDENCE - profile::MIN_EVIDENCE, 0);
    assert_eq!(p.center(P), 5);
}

#[test]
fn careless_slip_after_clean_clears_is_ignored() {
    let mut p = rooms_at_center(PlayerProfile::calibrated(4), P, 2, 0);
    assert_eq!(p.streak, 2);
    let quick = AdaptEvent::RoomFinished(RoomResult {
        deaths: 1,
        time_secs: 25.0,
        first_death_secs: Some(0.9),
        ..Default::default()
    });
    p = reduce(reduce(p, start(99, P, 4, 0)), quick);
    assert_eq!(p.recent.last().unwrap().outcome, Outcome::Careless);
    assert_eq!(p.streak, 2, "a slip doesn't break the streak");
    assert_eq!(p.assists, 0.0, "nor raise assists");
    // And it doesn't count as evidence: two more clean rooms make 4/4 → promote.
    let p = rooms_at_center(p, P, 2, 0);
    assert_eq!(p.center(P), 5);
}

#[test]
fn quick_death_without_a_clean_run_is_a_real_struggle() {
    let p = room(PlayerProfile::calibrated(4), P, 4, 1);
    let quick = AdaptEvent::RoomFinished(RoomResult { deaths: 1, first_death_secs: Some(0.9), ..Default::default() });
    let p = reduce(reduce(p, start(7, P, 4, 0)), quick);
    assert_eq!(p.recent.last().unwrap().outcome, Outcome::Struggle);
    // Two quick deaths after clean clears aren't a slip either.
    let p = rooms_at_center(PlayerProfile::calibrated(4), P, 3, 0);
    let two = AdaptEvent::RoomFinished(RoomResult { deaths: 2, first_death_secs: Some(0.9), ..Default::default() });
    let p = reduce(reduce(p, start(8, P, 4, 0)), two);
    assert_eq!(p.recent.last().unwrap().outcome, Outcome::Struggle);
    assert_eq!(p.streak, 0);
}

#[test]
fn expected_deaths_never_count_against_you() {
    let s = Skill::Stains;
    let mut p = PlayerProfile::calibrated(4);
    for i in 0..4 {
        p = reduce(reduce(p, start(i, s, 4, 2)), finish(2));
    }
    assert_eq!(p.center(s), 5, "dying exactly as the room expects is a clean clear");
    assert_eq!(p.assists, 0.0);
    assert_eq!(p.streak, 4);
    // One beyond expected is a struggle.
    let p = reduce(reduce(p, start(9, s, 5, 2)), finish(3));
    assert_eq!(p.recent.last().unwrap().outcome, Outcome::Struggle);
}

#[test]
fn slow_but_clean_is_never_penalized() {
    let mut p = PlayerProfile::calibrated(4);
    for i in 0..4 {
        p = reduce(
            reduce(p, start(i, P, 4, 0)),
            AdaptEvent::RoomFinished(RoomResult { deaths: 0, time_secs: 300.0, ..Default::default() }),
        );
    }
    assert_eq!(p.center(P), 5);
    assert_eq!(p.assists, 0.0);
}

#[test]
fn streak_is_display_only() {
    // A long streak alone (all stretch rooms, none at center) never promotes.
    let mut p = PlayerProfile::calibrated(4);
    for _ in 0..10 {
        p = room(p, P, 6, 0);
    }
    assert_eq!(p.streak, 10);
    assert_eq!(p.center(P), 4);
}

// ─── Frustration ─────────────────────────────────────────────────────────────

#[test]
fn many_deaths_eases_the_band_and_raises_assists() {
    let p = PlayerProfile::calibrated(6);
    let spread = p.skill(P).spread;
    let p = room(p, P, 6, 3);
    assert_eq!(p.center(P), 5);
    assert!(p.skill(P).spread < spread);
    assert!(p.assists >= profile::FRUSTRATION_ASSISTS);
    assert!(p.frustrated());
    assert!(p.cues.contains(&Cue::Encourage(FrustrationSignal::ManyDeaths)));
    assert!(p.cues.contains(&Cue::Eased(P, 5)));
}

#[test]
fn idling_after_a_death_is_frustration() {
    let p = PlayerProfile::calibrated(6);
    let p = reduce(
        reduce(p, start(0, P, 6, 0)),
        AdaptEvent::RoomFinished(RoomResult { deaths: 1, idle_after_death_secs: 16.0, ..Default::default() }),
    );
    assert!(p.cues.contains(&Cue::Encourage(FrustrationSignal::IdleAfterDeath)));
    assert_eq!(p.center(P), 5);
}

#[test]
fn restarting_a_room_repeatedly_is_frustration_once() {
    let mut p = reduce(PlayerProfile::calibrated(6), start(5, P, 6, 0));
    for _ in 0..2 {
        p = reduce(p, start(5, P, 6, 0));
        assert!(p.cues.is_empty());
    }
    p = reduce(p, start(5, P, 6, 0));
    assert!(p.cues.contains(&Cue::Encourage(FrustrationSignal::Restarts)));
    assert_eq!(p.center(P), 5);
    // Further restarts and the finish don't pile on.
    p = reduce(p, start(5, P, 6, 0));
    p = reduce(p, finish(4));
    assert!(!p.cues.iter().any(|c| matches!(c, Cue::Encourage(_))));
    assert_eq!(p.center(P), 5);
}

#[test]
fn live_frustration_is_handled_once_per_room() {
    let p = reduce(PlayerProfile::calibrated(6), start(1, P, 6, 0));
    let p = reduce(p, AdaptEvent::Frustrated(FrustrationSignal::IdleAfterDeath));
    assert_eq!(p.center(P), 5);
    let p = reduce(p, AdaptEvent::Frustrated(FrustrationSignal::IdleAfterDeath));
    assert!(p.cues.is_empty());
    assert_eq!(p.center(P), 5);
}

#[test]
fn frustration_on_a_stretch_room_keeps_the_center() {
    let p = room(PlayerProfile::calibrated(6), P, 8, 4);
    assert_eq!(p.center(P), 6, "the stretch was the problem, not the center");
    assert!(p.skill(P).spread < profile::DEFAULT_SPREAD, "fewer stretch rooms from now on");
    assert!(p.cues.contains(&Cue::Encourage(FrustrationSignal::ManyDeaths)));
}

#[test]
fn frustration_never_drops_below_band_1() {
    let p = room(PlayerProfile::calibrated(1), P, 1, 5);
    assert_eq!(p.center(P), 1);
    assert!(p.assists > 0.0);
}

// ─── Assists ─────────────────────────────────────────────────────────────────

#[test]
fn assists_fade_before_the_band_rises() {
    let mut p = PlayerProfile::calibrated(4);
    p.assists = 0.3;
    // Plenty of clean rooms at center: first the dial winds down, the band holds.
    let mut rooms = 0;
    while p.assists > profile::ASSIST_EPS {
        p = rooms_at_center(p, P, 1, 0);
        rooms += 1;
        if p.assists > profile::ASSIST_EPS {
            assert_eq!(p.center(P), 4, "no promotion while assists are on (room {rooms})");
        }
    }
    assert!(rooms >= 4);
    // Assisted clears weren't promotion evidence: it takes fresh unassisted rooms.
    let p = rooms_at_center(p, P, profile::MIN_EVIDENCE, 0);
    assert_eq!(p.center(P), 5);
}

#[test]
fn deaths_beyond_expected_raise_assists() {
    let p = room(PlayerProfile::calibrated(4), P, 4, 2);
    assert!((p.assists - 2.0 * profile::ASSIST_RISE).abs() < 1e-6);
    let levers = p.levers();
    assert!(levers.coyote_mult > 1.0);
    assert_eq!(AssistLevers::from_dial(0.0), AssistLevers::NONE);
}

// ─── Choosing the next room ──────────────────────────────────────────────────

#[test]
fn band_distribution_sums_to_one_everywhere() {
    for c in 1..=10 {
        for i in 0..=10 {
            let d = band_distribution(c, i as f32 / 10.0);
            let sum: f32 = d.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "center {c} spread {i}: {sum}");
            assert!(d.iter().all(|w| *w >= 0.0));
        }
    }
}

#[test]
fn band_distribution_center_weight_and_folding() {
    // Tight: nearly all at the center.
    assert!(band_distribution(5, 0.0)[4] > 0.9);
    // Wide: center still the single most likely band, but under half.
    let wide = band_distribution(5, 1.0);
    assert!(wide[4] < 0.5 && wide.iter().all(|w| *w <= wide[4]));
    // Folding at the bottom: band 1 never offers anything "below", the weight lands on 1..=3.
    let low = band_distribution(1, 1.0);
    assert!(low[0] > wide[4], "folded-back weight piles onto the edge");
    assert!(low[4..].iter().all(|w| *w == 0.0), "no band above center+3");
    // And at the top.
    let high = band_distribution(10, 1.0);
    assert!(high[..6].iter().all(|w| *w == 0.0));
    assert!(high[9] > wide[4]);
    // Symmetric in the middle.
    for k in 1..=3 {
        assert!((wide[4 - k] - wide[4 + k]).abs() < 1e-6);
    }
}

#[test]
fn sampling_follows_the_distribution() {
    let mut rng = StdRng::seed_from_u64(1);
    let d = band_distribution(5, 0.6);
    let mut counts = [0usize; 10];
    let n = 20_000;
    for _ in 0..n {
        counts[sample_band(&d, &mut rng) as usize - 1] += 1;
    }
    for i in 0..10 {
        let f = counts[i] as f32 / n as f32;
        assert!((f - d[i]).abs() < 0.015, "band {}: {f} vs {}", i + 1, d[i]);
    }
}

fn with_rates(rates: &[(Skill, usize, usize)]) -> PlayerProfile {
    // `clean` of `total` rooms per skill, played as stretch rooms so centers stay put.
    let mut p = PlayerProfile::calibrated(3);
    for &(s, clean, total) in rates {
        for i in 0..total {
            p = room(p, s, 6, if i < clean { 0 } else { 1 });
            p.assists = 0.0;
            p.frustration_cooldown = 0;
        }
    }
    p
}

#[test]
fn pick_skill_is_60_40_strengths_vs_growth() {
    let unlocked = [Skill::Precision, Skill::HazardTiming, Skill::Waltz, Skill::Grease];
    let p = with_rates(&[
        (Skill::Precision, 9, 10),
        (Skill::HazardTiming, 8, 10),
        (Skill::Waltz, 2, 10),
        (Skill::Grease, 3, 10),
    ]);
    assert!(!p.frustrated());
    let mut rng = StdRng::seed_from_u64(3);
    let n = 20_000;
    let strengths = (0..n)
        .filter(|_| matches!(pick_skill(&p, &unlocked, &mut rng), Skill::Precision | Skill::HazardTiming))
        .count();
    let share = strengths as f32 / n as f32;
    assert!((share - 0.6).abs() < 0.02, "strength share {share}");

    // Frustrated: 80/20.
    let mut f = p.clone();
    f.frustration_cooldown = 3;
    let strengths = (0..n)
        .filter(|_| matches!(pick_skill(&f, &unlocked, &mut rng), Skill::Precision | Skill::HazardTiming))
        .count();
    let share = strengths as f32 / n as f32;
    assert!((share - 0.8).abs() < 0.02, "frustrated strength share {share}");
}

#[test]
fn pick_skill_respects_unlocked() {
    let p = PlayerProfile::calibrated(3);
    let mut rng = StdRng::seed_from_u64(4);
    let unlocked = [Skill::Waltz, Skill::Stains, Skill::Grease];
    for _ in 0..500 {
        assert!(unlocked.contains(&pick_skill(&p, &unlocked, &mut rng)));
    }
    assert_eq!(pick_skill(&p, &[Skill::Grease], &mut rng), Skill::Grease);
    assert_eq!(pick_skill(&p, &[], &mut rng), Skill::Precision);
    for _ in 0..200 {
        let r = next_room(&p, &unlocked, &mut rng);
        assert!(unlocked.contains(&r.skill));
        assert!((1..=6).contains(&r.band), "within ±3 of center 3");
    }
}

#[test]
fn unlocking_a_skill_starts_it_near_demonstrated_ability() {
    let mut p = PlayerProfile::calibrated(1);
    for s in [Skill::Precision, Skill::HazardTiming] {
        p.skills[s.index()].center = 6;
        p = room(p, s, 9, 0);
    }
    let p = reduce(p, AdaptEvent::SkillUnlocked(Skill::Waltz));
    assert_eq!(p.center(Skill::Waltz), 5);
    // Already-played skills keep their band.
    let p = reduce(p, AdaptEvent::SkillUnlocked(Skill::Precision));
    assert_eq!(p.center(Skill::Precision), 6);
}

// ─── Calibration ─────────────────────────────────────────────────────────────

/// Run calibration with a fixed answer per probe band; returns (profile, probe bands).
fn calibrate(clean_up_to: u8, time_secs: f32) -> (PlayerProfile, Vec<u8>) {
    let mut p = PlayerProfile::new();
    let mut rng = StdRng::seed_from_u64(0);
    let mut bands = Vec::new();
    for i in 0..10 {
        if !p.calibrating() {
            break;
        }
        let r = next_room(&p, &Skill::ALL, &mut rng);
        assert_eq!(r.skill, calibration::CALIBRATION_SKILL);
        bands.push(r.band);
        let deaths = if r.band <= clean_up_to { 0 } else { 2 };
        p = reduce(p, start(i, r.skill, r.band, 0));
        p = reduce(
            p,
            AdaptEvent::RoomFinished(RoomResult { deaths, time_secs, first_death_secs: Some(5.0), ..Default::default() }),
        );
    }
    (p, bands)
}

#[test]
fn calibration_brackets_and_stops_early() {
    // Clean at 3, struggles at 5: bracketed after two rooms.
    let (p, bands) = calibrate(4, 30.0);
    assert_eq!(bands, vec![3, 5]);
    assert_eq!(p.center(Skill::Precision), 3);
    assert_eq!(p.center(Skill::Waltz), 2, "unseen skills start one lower");
    assert!(p.cues.iter().any(|c| matches!(c, Cue::Calibrated(_))));
    assert!(p.assists > 0.0 && p.assists < 0.3, "a struggle in placement leaves a little help on");
}

#[test]
fn calibration_climbs_for_strong_players() {
    let (p, bands) = calibrate(10, 30.0);
    assert_eq!(bands, vec![3, 5, 7]);
    assert_eq!(p.center(Skill::Precision), 7);
    assert_eq!(p.assists, 0.0);
}

#[test]
fn calibration_bottoms_out_with_assists() {
    let (p, bands) = calibrate(0, 30.0);
    assert_eq!(bands, vec![3, 2, 1]);
    assert_eq!(p.center(Skill::Precision), 1);
    assert!(p.assists >= calibration::NO_CLEAN_ASSISTS);
}

#[test]
fn calibration_uses_time_only_upward() {
    // Fast clean clears place one higher (still under the band they struggled at)...
    let (fast, _) = calibrate(10, 15.0);
    assert_eq!(fast.center(Skill::Precision), 8);
    // ...slow ones place the same as average, never lower.
    let (slow, _) = calibrate(10, 90.0);
    assert_eq!(slow.center(Skill::Precision), 7);
    let (fast_bracketed, _) = calibrate(4, 15.0);
    assert_eq!(fast_bracketed.center(Skill::Precision), 4, "3 clean fast, 5 struggled → 4");
}

// ─── Music mood ──────────────────────────────────────────────────────────────

#[test]
fn band_mood_follows_how_hard_the_player_pushes() {
    assert_eq!(band_mood(&PlayerProfile::calibrated(4)), BandMood::NEUTRAL);
    let flying = rooms_at_center(PlayerProfile::calibrated(4), P, 6, 0);
    let mut struggling = PlayerProfile::calibrated(4);
    for _ in 0..6 {
        struggling = room(struggling, Skill::HazardTiming, 4, 2);
    }
    let (f, s) = (band_mood(&flying), band_mood(&struggling));
    assert!(f.intensity > 0.6 && f.freedom > 0.5, "{f:?}");
    assert!(s.intensity < 0.3 && s.freedom < 0.3, "{s:?}");
    for m in [f, s] {
        assert!((0.0..=1.0).contains(&m.intensity) && (0.0..=1.0).contains(&m.freedom));
    }
}

// ─── Story mode ──────────────────────────────────────────────────────────────

#[test]
fn story_assists_rise_with_excess_deaths_and_fade_between_checkpoints() {
    let mut s = reduce_story(StoryAssist::new(0.0), StoryEvent::LevelStarted { expected_deaths_per_segment: 2 });
    s = reduce_story(s, StoryEvent::Died);
    s = reduce_story(s, StoryEvent::Died);
    assert_eq!(s.assists, 0.0, "deaths within expectation don't raise assists");
    s = reduce_story(s, StoryEvent::Died);
    s = reduce_story(s, StoryEvent::Died);
    assert!(s.assists > 0.0);
    let raised = s.assists;
    // That segment wasn't clean: no fade at the checkpoint.
    s = reduce_story(s, StoryEvent::CheckpointReached);
    assert_eq!(s.assists, raised);
    // Clean segments fade it back to zero.
    let mut n = 0;
    while s.assists > 0.0 {
        s = reduce_story(s, StoryEvent::Died);
        s = reduce_story(s, StoryEvent::CheckpointReached);
        n += 1;
        assert!(n < 10);
    }
    assert_eq!(s.levers(), AssistLevers::NONE);
}

#[test]
fn story_frustration_encourages_once_per_segment() {
    let mut s = reduce_story(StoryAssist::new(0.0), StoryEvent::LevelStarted { expected_deaths_per_segment: 0 });
    for _ in 0..2 {
        s = reduce_story(s, StoryEvent::Died);
        assert_eq!(s.encourage, None);
    }
    s = reduce_story(s, StoryEvent::Died);
    assert_eq!(s.encourage, Some(FrustrationSignal::ManyDeaths));
    let a = s.assists;
    s = reduce_story(s, StoryEvent::IdleAfterDeath { secs: 30.0 });
    assert_eq!(s.encourage, None, "already handled this segment");
    assert_eq!(s.assists, a);
    s = reduce_story(s, StoryEvent::CheckpointReached);
    s = reduce_story(s, StoryEvent::IdleAfterDeath { secs: 30.0 });
    assert_eq!(s.encourage, Some(FrustrationSignal::IdleAfterDeath));
    // Restarting the level over and over.
    let mut r = StoryAssist::new(0.0);
    for _ in 0..2 {
        r = reduce_story(r, StoryEvent::LevelRestarted);
        assert_eq!(r.encourage, None);
    }
    r = reduce_story(r, StoryEvent::LevelRestarted);
    assert_eq!(r.encourage, Some(FrustrationSignal::Restarts));
}

// ─── Simulator ───────────────────────────────────────────────────────────────

const SEEDS: u64 = 10;
const ROOMS: usize = 200;
/// Ping-pong band changes (reversal within `sim::OSC_SPAN` rooms) allowed per 200-room run.
const MAX_OSCILLATIONS: usize = 5;

fn runs(name: &str) -> Vec<sim::SimRun> {
    let p = sim::player(name).unwrap();
    (0..SEEDS).map(|seed| sim::run(p, seed, ROOMS)).collect()
}

#[test]
fn simulator_is_deterministic() {
    let p = sim::player("sloppy").unwrap();
    assert_eq!(sim::run(p, 9, 100).metrics(), sim::run(p, 9, 100).metrics());
}

#[test]
fn learner_bands_rise() {
    // After warm-up, against the skills unlocked by then (the others sit at placement until
    // unlocked); at the end, over every skill.
    let first: Vec<usize> = Skill::ALL.iter().filter(|s| sim::unlock_room(**s) <= sim::WARMUP).map(|s| s.index()).collect();
    for r in runs("learner") {
        let c = &r.rooms[sim::WARMUP].centers;
        let early = first.iter().map(|&k| c[k] as f32).sum::<f32>() / first.len() as f32;
        let late = r.profile.skills.iter().map(|s| s.center as f32).sum::<f32>() / Skill::COUNT as f32;
        assert!(late >= early + 1.0, "mean band should rise ≥1: {early:.1} → {late:.1}");
        let m = r.metrics();
        assert!(m.promotions >= m.demotions + 10, "{m:?}");
    }
}

#[test]
fn nervous_beginner_settles_in_a_comfortable_band() {
    for (seed, r) in runs("nervous_beginner").into_iter().enumerate() {
        let m = r.metrics();
        assert!(
            (0.55..=0.80).contains(&m.clean_after_warmup),
            "seed {seed}: clean after warm-up {:.2}",
            m.clean_after_warmup
        );
        assert!(m.maxed_assists < 0.2, "seed {seed}: assists maxed {:.0}% of rooms", 100.0 * m.maxed_assists);
    }
}

#[test]
fn no_profile_ping_pongs() {
    for p in sim::PLAYERS {
        for seed in 0..SEEDS {
            let m = sim::run(p, seed, ROOMS).metrics();
            assert!(m.oscillations <= MAX_OSCILLATIONS, "{} seed {seed}: {} oscillations", p.name, m.oscillations);
        }
    }
}

#[test]
fn every_profile_lands_near_the_target_on_average() {
    for p in sim::PLAYERS {
        let ms: Vec<sim::Metrics> = (0..SEEDS).map(|s| sim::run(p, s, ROOMS).metrics()).collect();
        let clean = ms.iter().map(|m| m.clean_after_warmup).sum::<f32>() / ms.len() as f32;
        // Target is 60–85%; a fast learner can outpace the evidence a little.
        assert!((0.58..=0.90).contains(&clean), "{}: {clean:.2}", p.name);
    }
}

#[test]
fn uneven_player_gets_different_bands_per_skill() {
    for r in runs("uneven") {
        let c = &r.profile;
        assert!(
            c.center(Skill::Precision) >= c.center(Skill::Waltz) + 3,
            "precision {} vs waltz {}",
            c.center(Skill::Precision),
            c.center(Skill::Waltz)
        );
    }
}

#[test]
fn strong_players_climb_and_drop_assists() {
    for name in ["precise", "speedrunner"] {
        for r in runs(name) {
            let m = r.metrics();
            assert!(m.mean_assists < 0.15, "{name}: assists {:.2}", m.mean_assists);
            assert!(r.profile.center(Skill::Precision) >= 5, "{name}: precision {}", r.profile.center(Skill::Precision));
        }
    }
}
