//! Pixel art, generated at startup from ASCII pixel grids (no image files).
//! Everything is 16x16 unless noted on the [`SpriteId`] variant.

use bevy::{asset::RenderAssetUsages, platform::collections::HashMap, prelude::*};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub fn plugin(app: &mut App) {
    app.add_systems(PreStartup, build_sprites);
}

/// Every sprite in the game. Animated ones have several frames (see [`Sprites::frames`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum SpriteId {
    // The hero: a swirly poo with googly eyes and a tiny cape. 16x16, feet at the bottom row.
    PooIdle,
    PooRun,
    PooJump,
    PooFall,
    /// Death animation frames (squash into a splat).
    PooSplat,
    // Gus the plumber: overalls, cap, mustache, plunger. 16x16 (he's short), feet at the bottom.
    GusIdle,
    GusRun,
    GusJump,
    /// Golden nugget, spinning frames. 16x16 with the nugget ~8px centered.
    Nugget,
    /// Checkpoint: toilet-paper holder, untouched / unrolled after touching. 16x16.
    CheckpointOff,
    CheckpointOn,
    /// Goal: a plunger stuck in the ground with a waving flag. 16x32 (two tiles tall), frames wave.
    GoalFlag,
    /// Decorative throne next to the goal. 32x32.
    Throne,
    /// Floor spikes (row of upturned toilet brushes). 16x16, bristles in the bottom half.
    SpikesUp,
    /// Ceiling spikes. 16x16, top half.
    SpikesDown,
    /// Deadly liquid surface tile (animated ripple) and fill tile, per world tint.
    LiquidTop(u8),
    LiquidFill(u8),
    /// Fly, wing-flap frames. 8x8.
    Fly,
    /// Air-freshener can (on the floor), 16x16; and the spray jet segment (animated), 16x16.
    SprayCan,
    SprayJet,
    /// Moving platform segments, one per tile of platform width. 16x16 (art in the top ~8px).
    PlatformTp,
    PlatformDuck,
    PlatformPlunger,
    /// Ground tiles per world (1..=5): grass-equivalent top surface, and interior fill.
    GroundTop(u8),
    GroundFill(u8),
    /// One-way platform per world. 16x16, art in the top ~5px.
    OneWay(u8),
    /// Full-screen backdrop per world: 256x144, tiled horizontally, drawn with parallax.
    Backdrop(u8),
    /// Small square particle (white; tint it). 2x2.
    Particle,
    /// Green "toot" cloud puff for the double jump, frames grow and fade. 16x16.
    TootPuff,
    /// UI icons: nugget 8x8, lock 8x8.
    IconNugget,
    IconLock,
}

#[derive(Resource, Debug, Default)]
pub struct Sprites {
    map: HashMap<SpriteId, Vec<Handle<Image>>>,
}

impl Sprites {
    pub fn frames(&self, id: SpriteId) -> &[Handle<Image>] {
        self.map.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Frame `i`, wrapping. Missing sprites give the default (white) image.
    pub fn frame(&self, id: SpriteId, i: usize) -> Handle<Image> {
        let f = self.frames(id);
        if f.is_empty() { Handle::default() } else { f[i % f.len()].clone() }
    }

    pub fn get(&self, id: SpriteId) -> Handle<Image> {
        self.frame(id, 0)
    }
}

/// Placeholder: a solid magenta square per sprite, until the real art lands.
fn build_sprites(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let mut sprites = Sprites::default();
    let mut add = |id: SpriteId, w: u32, h: u32| {
        let img = Image::new_fill(
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            &[255, 0, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        sprites.map.entry(id).or_default().push(images.add(img));
    };
    use SpriteId::*;
    for id in [PooIdle, PooRun, PooJump, PooFall, PooSplat, GusIdle, GusRun, GusJump, Nugget,
        CheckpointOff, CheckpointOn, SpikesUp, SpikesDown, SprayCan, SprayJet, PlatformTp,
        PlatformDuck, PlatformPlunger, TootPuff]
    {
        add(id, 16, 16);
    }
    add(GoalFlag, 16, 32);
    add(Throne, 32, 32);
    add(Fly, 8, 8);
    add(Particle, 2, 2);
    add(IconNugget, 8, 8);
    add(IconLock, 8, 8);
    for w in 1..=5 {
        for id in [GroundTop(w), GroundFill(w), OneWay(w), LiquidTop(w), LiquidFill(w)] {
            add(id, 16, 16);
        }
        add(Backdrop(w), 256, 144);
    }
    commands.insert_resource(sprites);
}
