//! Pixel art, generated at startup from ASCII pixel grids (no image files).
//! Everything is 16x16 unless noted on the [`SpriteId`] variant.

use bevy::{asset::RenderAssetUsages, platform::collections::HashMap, prelude::*};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

mod backdrops;
mod characters;
mod items;
pub mod palette;
mod tiles;

pub use palette::Pixels;

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
    // Han the plumber: overalls, cap, mustache, plunger. 16x16 (he's short), feet at the bottom.
    HanIdle,
    HanRun,
    HanJump,
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

impl SpriteId {
    /// Every sprite, including each world's variants (1..=5). Handy for tests and tools.
    pub fn all() -> Vec<SpriteId> {
        use SpriteId::*;
        let mut v = vec![
            PooIdle, PooRun, PooJump, PooFall, PooSplat, HanIdle, HanRun, HanJump, Nugget,
            CheckpointOff, CheckpointOn, GoalFlag, Throne, SpikesUp, SpikesDown, Fly, SprayCan,
            SprayJet, PlatformTp, PlatformDuck, PlatformPlunger, Particle, TootPuff, IconNugget,
            IconLock,
        ];
        for w in tiles::WORLDS {
            v.extend([GroundTop(w), GroundFill(w), OneWay(w), LiquidTop(w), LiquidFill(w), Backdrop(w)]);
        }
        v
    }

    /// Documented size of every frame of this sprite, in pixels.
    pub fn size(self) -> (u32, u32) {
        use SpriteId::*;
        match self {
            GoalFlag => (16, 32),
            Throne => (32, 32),
            Fly | IconNugget | IconLock => (8, 8),
            Particle => (2, 2),
            Backdrop(_) => (backdrops::W as u32, backdrops::H as u32),
            _ => (16, 16),
        }
    }
}

/// Render every frame of a sprite as plain RGBA8 (sRGB) pixels. Pure: no Bevy app needed.
/// World numbers outside 1..=5 are clamped.
pub fn render(id: SpriteId) -> Vec<Pixels> {
    use SpriteId::*;
    match id {
        PooIdle => characters::poo_idle(),
        PooRun => characters::poo_run(),
        PooJump => characters::poo_jump(),
        PooFall => characters::poo_fall(),
        PooSplat => characters::poo_splat(),
        HanIdle => characters::han_idle(),
        HanRun => characters::han_run(),
        HanJump => characters::han_jump(),
        Nugget => items::nugget(),
        CheckpointOff => items::checkpoint_off(),
        CheckpointOn => items::checkpoint_on(),
        GoalFlag => items::goal_flag(),
        Throne => items::throne(),
        SpikesUp => items::spikes_up(),
        SpikesDown => items::spikes_down(),
        LiquidTop(w) => tiles::liquid_top(w),
        LiquidFill(w) => tiles::liquid_fill(w),
        Fly => items::fly(),
        SprayCan => items::spray_can(),
        SprayJet => items::spray_jet(),
        PlatformTp => items::platform_tp(),
        PlatformDuck => items::platform_duck(),
        PlatformPlunger => items::platform_plunger(),
        GroundTop(w) => tiles::ground_top(w),
        GroundFill(w) => tiles::ground_fill(w),
        OneWay(w) => tiles::one_way(w),
        Backdrop(w) => backdrops::backdrop(w),
        Particle => items::particle(),
        TootPuff => items::toot_puff(),
        IconNugget => items::icon_nugget(),
        IconLock => items::icon_lock(),
    }
}

fn build_sprites(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let mut sprites = Sprites::default();
    for id in SpriteId::all() {
        let frames = render(id)
            .into_iter()
            .map(|px| {
                images.add(Image::new(
                    Extent3d { width: px.w, height: px.h, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    px.data,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::RENDER_WORLD,
                ))
            })
            .collect();
        sprites.map.insert(id, frames);
    }
    commands.insert_resource(sprites);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sprite_has_frames_of_the_documented_size() {
        for id in SpriteId::all() {
            let frames = render(id);
            assert!(!frames.is_empty(), "{id:?} has no frames");
            for (i, f) in frames.iter().enumerate() {
                assert_eq!((f.w, f.h), id.size(), "{id:?} frame {i}");
                assert_eq!(f.data.len(), (f.w * f.h * 4) as usize);
                assert!(f.data.chunks_exact(4).any(|px| px[3] > 0), "{id:?} frame {i} is blank");
            }
        }
    }

    #[test]
    fn every_world_is_covered() {
        let all = SpriteId::all();
        for w in 1..=5 {
            for id in [
                SpriteId::GroundTop(w),
                SpriteId::GroundFill(w),
                SpriteId::OneWay(w),
                SpriteId::LiquidTop(w),
                SpriteId::LiquidFill(w),
                SpriteId::Backdrop(w),
            ] {
                assert!(all.contains(&id), "{id:?} missing");
            }
        }
    }

    #[test]
    fn animation_frame_counts() {
        use SpriteId::*;
        let n = |id| render(id).len();
        assert_eq!(n(PooRun), 4);
        assert_eq!(n(PooIdle), 2);
        assert_eq!(n(PooJump), 1);
        assert_eq!(n(PooFall), 1);
        assert_eq!(n(PooSplat), 3);
        assert_eq!(n(HanIdle), 2);
        assert_eq!(n(HanRun), 4);
        assert_eq!(n(HanJump), 1);
        assert_eq!(n(Nugget), 4);
        assert!((2..=3).contains(&n(GoalFlag)));
        assert_eq!(n(Fly), 2);
        assert_eq!(n(SprayJet), 2);
        assert!((3..=4).contains(&n(TootPuff)));
        for w in tiles::WORLDS {
            assert!((2..=3).contains(&n(LiquidTop(w))));
        }
    }

    #[test]
    fn grids_are_rectangular_and_use_only_palette_chars() {
        let all = characters::grids().into_iter().chain(items::grids()).chain(tiles::grids());
        for (name, rows, pal) in all {
            if let Err(e) = palette::try_grid(&rows, pal) {
                panic!("grid {name}: {e:?}");
            }
        }
    }

    #[test]
    fn sprites_keep_a_small_palette() {
        for id in SpriteId::all() {
            if matches!(id, SpriteId::Backdrop(_)) {
                continue;
            }
            for f in render(id) {
                assert!(f.colour_count() <= 12, "{id:?} uses {} colours", f.colour_count());
            }
        }
    }

    /// Ground fill tiles in all directions and tops horizontally: their edges must be fully
    /// opaque so no gaps show between neighbours.
    #[test]
    fn ground_tiles_are_solid_at_the_edges() {
        for w in tiles::WORLDS {
            for id in [SpriteId::GroundTop(w), SpriteId::GroundFill(w), SpriteId::LiquidFill(w)] {
                let f = &render(id)[0];
                for i in 0..16 {
                    assert_eq!(f.get(0, i)[3], 255, "{id:?}");
                    assert_eq!(f.get(15, i)[3], 255, "{id:?}");
                    assert_eq!(f.get(i, 15)[3], 255, "{id:?}");
                    assert_eq!(f.get(i, 0)[3], 255, "{id:?}");
                }
            }
        }
    }
}
