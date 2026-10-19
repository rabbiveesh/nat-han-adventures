//! A level's secret (`secret@` in the level file, NAT ALONE): Han glimpsed through a grate.
//! World 3 has Nat lost alone; now and then, somewhere hidden, he sees Han on the other side of
//! a wall. The first time in a level visit Nat comes close, Han barks the line ("Nat?!"), a
//! little fanfare plays and [`SecretFound`] tells the save (`Progress::solo_secrets`).
//!
//! Also the level's `shout:` column: from there on the band plays shout choruses
//! ([`ShoutChorus`], the audio plugin forces them).

use bevy::prelude::*;

use super::physics::Dead;
use super::{ActiveLevel, GameSet, LevelEntity, Player, Pos};
use crate::audio::Sfx;
use crate::events::{HanSays, LevelStarted, PlaySfx};
use crate::level::{SECRET_RADIUS, TILE};

/// Han behind the secret's grate: just a picture (no brain, no body). `found` once Nat's seen
/// him this visit (then his speech bubble shows over him). Pos is his middle, like Han's.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HanGlimpse {
    pub text: String,
    pub found: bool,
}

/// Nat found the level's secret (once per level visit).
#[derive(Message, Debug, Clone, Copy)]
pub struct SecretFound;

/// The band plays shout choruses from now on (`true`), or as it likes again (`false`: a level
/// start).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShoutChorus(pub bool);

/// Past the level's `shout:` column this visit.
#[derive(Resource, Debug, Default)]
struct Shouting(bool);

pub(super) fn plugin(app: &mut App) {
    app.register_type::<HanGlimpse>()
        .add_message::<SecretFound>()
        .add_message::<ShoutChorus>()
        .init_resource::<Shouting>()
        .add_systems(Update, (spawn_glimpse, reset_shout, record_secret))
        .add_systems(FixedUpdate, (find_secret, shout).in_set(GameSet::Interact));
}

/// The glimpse comes with its level (a restart brings back an unfound one).
fn spawn_glimpse(mut commands: Commands, active: Option<Res<ActiveLevel>>, glimpse: Query<(), With<HanGlimpse>>) {
    let Some(active) = active else { return };
    let Some(s) = &active.level.secret else { return };
    if !glimpse.is_empty() {
        return;
    }
    let at = active.level.tile_center(s.col, s.row);
    commands.spawn((
        Name::new("HanGlimpse"),
        LevelEntity,
        HanGlimpse { text: s.text.clone(), found: false },
        Pos(at),
        Transform::from_translation(at.extend(0.5)),
    ));
}

fn find_secret(
    player: Query<&Pos, (With<Player>, Without<Dead>)>,
    mut glimpse: Query<(&Pos, &mut HanGlimpse)>,
    mut says: MessageWriter<HanSays>,
    mut sfx: MessageWriter<PlaySfx>,
    mut found: MessageWriter<SecretFound>,
) {
    let Ok(nat) = player.single() else { return };
    for (pos, mut g) in &mut glimpse {
        // The grate's center: half a tile above his middle.
        if !g.found && (pos.0 + Vec2::Y * TILE / 2.0).distance(nat.0) <= SECRET_RADIUS {
            g.found = true;
            says.write(HanSays { text: g.text.clone() });
            sfx.write(PlaySfx(Sfx::Secret));
            found.write(SecretFound);
        }
    }
}

fn shout(
    active: Res<ActiveLevel>,
    player: Query<&Pos, With<Player>>,
    mut shouting: ResMut<Shouting>,
    mut out: MessageWriter<ShoutChorus>,
) {
    let (Some(col), Ok(nat)) = (active.level.shout, player.single()) else { return };
    if !shouting.0 && nat.0.x >= col as f32 * TILE {
        shouting.0 = true;
        out.write(ShoutChorus(true));
    }
}

fn reset_shout(mut started: MessageReader<LevelStarted>, mut shouting: ResMut<Shouting>, mut out: MessageWriter<ShoutChorus>) {
    if started.read().count() > 0 {
        if shouting.0 {
            out.write(ShoutChorus(false));
        }
        shouting.0 = false;
    }
}

/// The save remembers each NAT ALONE level's secret once found.
fn record_secret(
    mut found: MessageReader<SecretFound>,
    solo: Option<Res<super::solo::SoloRun>>,
    mut progress: ResMut<crate::save::Progress>,
) {
    if found.read().count() > 0
        && let Some(n) = solo
        && !progress.solo_secrets[n.0]
    {
        progress.solo_secrets[n.0] = true;
    }
}
