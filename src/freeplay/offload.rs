//! Free-play rooms validated in a Web Worker (the web build).
//!
//! On web the game has one thread, so each validation attempt ([`super::generate::Job::step`],
//! ~20–60 ms) is a hitch. The web build ships a second, small wasm (`src/bin/roomgen.rs`,
//! loaded by Trunk as a worker, see `index.html`) that runs whole [`Job`]s instead. Only text
//! crosses the boundary: the job goes over as its seed and [`RoomPlan`], and the answer comes
//! back as the [`Verdict`] (attempts, deaths, gate marks) and a fingerprint of the room. The
//! game draws that attempt again ([`Room::rebuild`]: drawing is cheap and seeded) and keeps it
//! only if the fingerprint matches; anything else (no worker, a worker error, a stale cached
//! worker from an older deploy) and the room is generated on the main thread as before.
//!
//! The worker says [`READY`] once its wasm is running: a message posted before then would be
//! dropped unanswered (the worker has no handler yet), so jobs wait in the [`Link`]'s outbox
//! until it does. A worker that stays silent past [`PATIENCE_SECS`] is retired the same way.
//!
//! The protocol is pure and checked natively (tests below, `tests/freeplay.rs`); [`send`] and
//! [`recv`] are the browser glue (no-ops elsewhere), [`worker_main`] the worker's side.

use std::collections::VecDeque;

use super::generate::{CALIBRATION_LINES, CHECKPOINT_LINES, Job, Room, RoomPlan, Verdict};
use crate::adapt::{AssistLevers, RoomRequest, Skill};
use crate::level::{GateMark, Level, Topic};

/// First word of every message; bump it when the format changes.
pub const PROTOCOL: &str = "nh1";

/// The worker's hello, once it can take jobs.
pub const READY: &str = "nh1 ready";

/// How long a job may go unanswered (the worker never started, or hung) before the game gives
/// up on the worker and generates the room itself. A whole job takes a few seconds at worst
/// on a slow phone; Nat needs the room by the end of the one he's in.
pub const PATIENCE_SECS: f32 = 6.0;

/// The game's end of the worker link: jobs are held until the worker is [`READY`].
#[derive(Debug, Default)]
pub struct Link {
    ready: bool,
    outbox: VecDeque<String>,
    inbox: VecDeque<String>,
}

impl Link {
    /// A job to send: what to post to the worker now (nothing yet if it isn't ready).
    pub fn send(&mut self, text: String) -> Option<String> {
        if self.ready {
            return Some(text);
        }
        self.outbox.push_back(text);
        None
    }

    /// A message from the worker: what to post to it now (the jobs held until it was ready).
    pub fn heard(&mut self, text: String) -> Vec<String> {
        if text == READY {
            self.ready = true;
            return self.outbox.drain(..).collect();
        }
        self.inbox.push_back(text);
        Vec::new()
    }

    /// The worker's next answer.
    pub fn next(&mut self) -> Option<String> {
        self.inbox.pop_front()
    }
}

/// A stable fingerprint of a room (FNV-1a over its debug form): the worker's room and the
/// game's redraw of it must be the same room, field for field.
pub fn fingerprint(level: &Level) -> u64 {
    let text = format!("{level:?}");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn line_id(line: &str) -> Option<usize> {
    CHECKPOINT_LINES.iter().chain(CALIBRATION_LINES).position(|l| *l == line)
}

fn line_at(id: usize) -> Option<&'static str> {
    CHECKPOINT_LINES.iter().chain(CALIBRATION_LINES).nth(id).copied()
}

/// A job for the worker: `id` comes back with the answer. `None` if the plan can't be sent
/// (a checkpoint line from outside the tables).
pub fn encode_job(id: u64, seed: u32, plan: &RoomPlan) -> Option<String> {
    let skill = Skill::ALL.iter().position(|s| *s == plan.request.skill)?;
    let a = &plan.request.assists;
    Some(format!(
        "{PROTOCOL} job {id} {seed} {} {skill} {} {} {} {} {} {} {} {} {} {} {}",
        plan.index,
        plan.request.band,
        a.coyote_mult,
        a.jump_buffer_mult,
        a.hitbox_forgiveness_px,
        a.extra_checkpoint as u8,
        a.han_hint as u8,
        a.extra_nuggets_before_quartal,
        plan.template,
        plan.hint as u8,
        line_id(plan.line)?,
        plan.world,
    ))
}

/// The words after `<PROTOCOL> <kind>`.
fn words<'a>(text: &'a str, kind: &str) -> Option<std::str::SplitWhitespace<'a>> {
    let mut w = text.split_whitespace();
    (w.next()? == PROTOCOL && w.next()? == kind).then_some(w)
}

fn flag(w: &str) -> Option<bool> {
    match w {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub fn decode_job(text: &str) -> Option<(u64, u32, RoomPlan)> {
    let mut w = words(text, "job")?;
    let mut next = || w.next();
    let id = next()?.parse().ok()?;
    let seed = next()?.parse().ok()?;
    let index = next()?.parse().ok()?;
    let skill = *Skill::ALL.get(next()?.parse::<usize>().ok()?)?;
    let band = next()?.parse().ok()?;
    let assists = AssistLevers {
        coyote_mult: next()?.parse().ok()?,
        jump_buffer_mult: next()?.parse().ok()?,
        hitbox_forgiveness_px: next()?.parse().ok()?,
        extra_checkpoint: flag(next()?)?,
        han_hint: flag(next()?)?,
        extra_nuggets_before_quartal: next()?.parse().ok()?,
    };
    let template = next()?.parse().ok()?;
    let hint = flag(next()?)?;
    let line = line_at(next()?.parse().ok()?)?;
    let world = next()?.parse().ok()?;
    if next().is_some() || template >= super::templates::TEMPLATES.len() {
        return None;
    }
    let request = RoomRequest { skill, band, assists };
    Some((id, seed, RoomPlan { index, request, template, hint, line, world }))
}

/// The worker's answer to job `id`: what it found and the room's [`fingerprint`].
pub fn encode_verdict(id: u64, v: &Verdict, fingerprint: u64) -> String {
    let mut out = format!(
        "{PROTOCOL} room {id} {} {} {} {} {fingerprint} {}",
        v.attempts,
        v.fallback as u8,
        v.deaths,
        v.micros,
        v.marks.len()
    );
    for m in &v.marks {
        out += &format!(" {} {} {} {} {}", m.topic.word(), m.c0, m.r0, m.c1, m.r1);
    }
    out
}

pub fn decode_verdict(text: &str) -> Option<(u64, Verdict, u64)> {
    let mut w = words(text, "room")?;
    let mut next = || w.next();
    let id = next()?.parse().ok()?;
    let attempts = next()?.parse().ok()?;
    let fallback = flag(next()?)?;
    let deaths = next()?.parse().ok()?;
    let micros = next()?.parse().ok()?;
    let fingerprint = next()?.parse().ok()?;
    let n: usize = next()?.parse().ok()?;
    let mut marks = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        marks.push(GateMark {
            topic: Topic::from_word(next()?)?,
            c0: next()?.parse().ok()?,
            r0: next()?.parse().ok()?,
            c1: next()?.parse().ok()?,
            r1: next()?.parse().ok()?,
        });
    }
    if next().is_some() || attempts == 0 {
        return None;
    }
    Some((id, Verdict { attempts, fallback, deaths, marks, micros }, fingerprint))
}

/// The worker's whole job: a message in, the answer out (`<PROTOCOL> error` for a message it
/// can't read, so the game falls back).
pub fn answer(text: &str) -> String {
    let Some((id, seed, plan)) = decode_job(text) else {
        return format!("{PROTOCOL} error");
    };
    let room = Job::new(seed, plan).run();
    encode_verdict(id, &room.verdict(), fingerprint(&room.level))
}

/// What became of a job sent to the worker.
#[derive(Debug)]
pub enum Outcome {
    /// Still working.
    Pending,
    /// The room, rebuilt and matching the worker's.
    Done(Box<Room>),
    /// No usable answer: generate it here instead.
    Failed,
}

/// Read the worker's answer `text` to job `id` (`seed`, `plan`).
pub fn accept(id: u64, seed: u32, plan: &RoomPlan, text: &str) -> Option<Room> {
    let (got, verdict, fp) = decode_verdict(text)?;
    if got != id {
        return None;
    }
    let room = Room::rebuild(seed, plan.clone(), verdict);
    (fingerprint(&room.level) == fp).then_some(room)
}

/// Check on job `id`, sent `waited` seconds ago: answers to older jobs are dropped; an
/// unreadable or mismatched answer, a worker that broke, or no answer within
/// [`PATIENCE_SECS`] is [`Outcome::Failed`] and retires the worker (later rooms are generated
/// on the main thread).
pub fn poll(id: u64, seed: u32, plan: &RoomPlan, waited: f32) -> Outcome {
    while let Some(text) = recv() {
        if decode_verdict(&text).is_some_and(|(got, ..)| got != id) {
            continue;
        }
        return match accept(id, seed, plan, &text) {
            Some(room) => Outcome::Done(Box::new(room)),
            None => {
                web::retire(&format!("bad answer: {}", text.chars().take(80).collect::<String>()));
                Outcome::Failed
            }
        };
    }
    if web::broken() {
        return Outcome::Failed;
    }
    if waited > PATIENCE_SECS {
        web::retire(&format!("no answer in {waited:.1} s"));
        return Outcome::Failed;
    }
    Outcome::Pending
}

/// Hand a job to the worker (started on first use). False if there's no worker to take it.
pub fn send(text: &str) -> bool {
    web::send(text)
}

/// The worker's next message, if one came.
pub fn recv() -> Option<String> {
    web::recv()
}

#[cfg(not(target_arch = "wasm32"))]
mod web {
    pub fn send(_: &str) -> bool {
        false
    }
    pub fn recv() -> Option<String> {
        None
    }
    pub fn broken() -> bool {
        true
    }
    pub fn retire(_: &str) {}
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    /// Trunk's loader for the `roomgen` worker (worker files keep their plain names).
    const URL: &str = "./roomgen_loader.js";

    #[derive(Default)]
    struct State {
        started: bool,
        worker: Option<web_sys::Worker>,
        link: Rc<RefCell<super::Link>>,
        broken: Rc<RefCell<bool>>,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::default();
    }

    fn start(s: &mut State) {
        s.started = true;
        let worker = match web_sys::Worker::new(URL) {
            Ok(w) => w,
            Err(e) => {
                bevy::log::warn!("free play: no room worker ({e:?}); rooms are generated in the game");
                *s.broken.borrow_mut() = true;
                return;
            }
        };
        let link = s.link.clone();
        let to = worker.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let Some(text) = e.data().as_string() else { return };
            // Jobs held until the worker was ready go now.
            for job in link.borrow_mut().heard(text) {
                let _ = to.post_message(&JsValue::from_str(&job));
            }
        });
        let broken = s.broken.clone();
        let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
            let msg = e.dyn_ref::<web_sys::ErrorEvent>().map(|e| e.message()).unwrap_or_default();
            bevy::log::warn!("free play: the room worker failed ({msg}); rooms are generated in the game");
            *broken.borrow_mut() = true;
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        // The worker lives as long as the page.
        on_message.forget();
        on_error.forget();
        s.worker = Some(worker);
    }

    pub fn send(text: &str) -> bool {
        STATE.with_borrow_mut(|s| {
            if !s.started {
                start(s);
            }
            if *s.broken.borrow() {
                return false;
            }
            let Some(w) = &s.worker else { return false };
            match s.link.borrow_mut().send(text.to_string()) {
                Some(now) => w.post_message(&JsValue::from_str(&now)).is_ok(),
                None => true,
            }
        })
    }

    pub fn recv() -> Option<String> {
        STATE.with_borrow(|s| s.link.borrow_mut().next())
    }

    pub fn broken() -> bool {
        STATE.with_borrow(|s| *s.broken.borrow())
    }

    pub fn retire(why: &str) {
        bevy::log::warn!("free play: retiring the room worker ({why}); rooms are generated in the game");
        STATE.with_borrow_mut(|s| {
            *s.broken.borrow_mut() = true;
            if let Some(w) = s.worker.take() {
                w.terminate();
            }
        });
    }

    /// Inside the worker: answer every job message.
    pub fn serve() {
        let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
        let reply_to = scope.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            let text = e.data().as_string().unwrap_or_default();
            let _ = reply_to.post_message(&JsValue::from_str(&super::answer(&text)));
        });
        scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        // Jobs posted before this point were dropped: the game holds them until it hears this.
        let _ = scope.post_message(&JsValue::from_str(super::READY));
    }
}

/// The worker binary's entry point (`src/bin/roomgen.rs`): serve jobs until the page closes.
#[cfg(target_arch = "wasm32")]
pub fn worker_main() {
    web::serve();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapt::RoomRequest;

    fn plan(skill: Skill, band: u8, index: u32) -> RoomPlan {
        let assists = AssistLevers {
            coyote_mult: 1.37,
            jump_buffer_mult: 1.1,
            hitbox_forgiveness_px: 2.5,
            extra_checkpoint: true,
            han_hint: false,
            extra_nuggets_before_quartal: 2,
        };
        RoomPlan::new(4242, index, RoomRequest { skill, band, assists }, 3, index > 0, index == 0, 10)
    }

    #[test]
    fn jobs_round_trip() {
        for (i, s) in Skill::ALL.into_iter().enumerate() {
            let p = plan(s, 1 + i as u8, i as u32);
            let text = encode_job(77, 4242, &p).unwrap();
            assert_eq!(decode_job(&text), Some((77, 4242, p)), "{text}");
        }
        assert_eq!(decode_job("nh0 job 1 2"), None);
        assert_eq!(decode_job(&(encode_job(1, 2, &plan(Skill::Grease, 3, 1)).unwrap() + " 9")), None);
        assert!(answer("garbage").ends_with(" error"));
    }

    /// The first room's job used to go out the moment the worker was created, before its wasm
    /// had loaded: the worker dropped it, never answered, and free play waited forever behind
    /// room 1's capped pipe. Jobs now wait for the worker's hello.
    #[test]
    fn jobs_wait_for_the_worker_to_be_ready() {
        let mut link = Link::default();
        assert_eq!(link.send("a".into()), None);
        assert_eq!(link.send("b".into()), None);
        assert_eq!(link.next(), None);
        assert_eq!(link.heard(READY.into()), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(link.send("c".into()), Some("c".into()));
        assert_eq!(link.heard("nh1 room 1".into()), Vec::<String>::new());
        assert_eq!(link.next().as_deref(), Some("nh1 room 1"));
        assert_eq!(link.next(), None);
        // The hello is never mistaken for an answer.
        assert!(decode_verdict(READY).is_none());
    }

    #[test]
    fn the_worker_and_the_game_build_the_same_room() {
        for (i, s) in [Skill::Precision, Skill::Stains, Skill::Waltz].into_iter().enumerate() {
            let p = plan(s, 4, 1 + i as u32);
            let reply = answer(&encode_job(9, 4242, &p).unwrap());
            let room = accept(9, 4242, &p, &reply).expect("matching room");
            let direct = Job::new(4242, p.clone()).run();
            assert_eq!(room.level, direct.level);
            assert_eq!((room.plan, room.attempts, room.fallback), (direct.plan, direct.attempts, direct.fallback));
            assert_eq!(room.expected_deaths, direct.expected_deaths);
            // Someone else's answer, or a different room (a stale worker), is refused.
            assert!(accept(10, 4242, &p, &reply).is_none());
            let (_, v, fp) = decode_verdict(&reply).unwrap();
            assert!(accept(9, 4242, &p, &encode_verdict(9, &v, fp ^ 1)).is_none());
            let other = Room::rebuild(4243, p.clone(), v);
            assert_eq!(accept(9, 4243, &p, &reply).is_some(), other.level == room.level);
        }
    }
}
