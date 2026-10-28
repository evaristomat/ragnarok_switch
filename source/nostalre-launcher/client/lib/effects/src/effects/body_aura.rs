//! Effects around a unit's body: orbs circling it, an aura underfoot, a glow around its outline, motes rising off
//! it. The counters kRO draws (spirit spheres, Millennium Shield) are orbs; nostalRE's own rules
//! (`custom.skill.body_effect`) combine any of the four (`docs/nostalre-body-effects.md`).

use crate::draw::{BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect_trait::{
    BodyCopy, BodyGlow, CameraView, Effect, EffectRenderCtx, EffectUpdateCtx,
};
use crate::own_textures;

const FRAMES_PER_SECOND: f32 = 60.0;
/// A body effect (`EffectId::BodyAura`) is held until the counter or status that put it on takes it off: its key
/// despawns it, never a clock (a Kagerou's charms last five minutes, a Monk's spheres ten)
pub const HELD_MS: u32 = u32::MAX;
/// The spirit spheres the server plays by effect id (`EF_CHOOKGI`), with no key to take them off
pub const TOTAL_DURATION_MS: u32 = 99990;

const SQRT2: f32 = std::f32::consts::SQRT_2;
/// The most orbs a layer may draw (a Soul Reaper's 20 souls)
pub const MAX_ORBS: u8 = 20;
/// A Monk's spheres turn one degree a frame (speed 1)
const ORBIT_DEG_PER_FRAME: f32 = 1.0;
/// Every layer fades in over these frames, up to its intensity (a Monk's spheres: one 255th a frame to 200)
const FADE_IN_FRAMES: f32 = 200.0;
/// Where motes start, up the body from the feet
const MOTE_BODY_HEIGHT: f32 = 12.0;
const MAX_MOTES: usize = 64;
const GROUND_BREATH_SECS: f32 = 2.0;
/// A swollen orb's halo, its side a share of the orb's
const HALO_SHARE: f32 = 1.6;

/// A texture for orbs and motes, from a fixed set so a rule cannot name a missing one
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotLook {
    /// A round glow with a hard edge (a Monk's spheres)
    Glow,
    /// A white starburst
    Spark,
    /// A soft white glow
    Soft,
    /// kRO's green torch flame, animated (a Soul Reaper's souls)
    GreenFlame,
    /// kRO's red torch flame, animated
    RedFlame,
    /// kRO's charm sprites, drawn as they are (a Kagerou's or Oboro's charms): water, earth, fire, wind
    WaterOrb,
    StoneOrb,
    FireOrb,
    LightningOrb,
    /// nostalRE's own charms, pixel-art orbs drawn as they are: fire, water, earth, wind
    FireCharm,
    WaterCharm,
    EarthCharm,
    WindCharm,
}

pub const GREEN_FLAME: [&str; 13] = [
    "torch_green01.bmp",
    "torch_green02.bmp",
    "torch_green03.bmp",
    "torch_green04.bmp",
    "torch_green05.bmp",
    "torch_green06.bmp",
    "torch_green07.bmp",
    "torch_green08.bmp",
    "torch_green09.bmp",
    "torch_green10.bmp",
    "torch_green11.bmp",
    "torch_green12.bmp",
    "torch_green13.bmp",
];
pub const RED_FLAME: [&str; 13] = [
    "torch_red01.bmp",
    "torch_red02.bmp",
    "torch_red03.bmp",
    "torch_red04.bmp",
    "torch_red05.bmp",
    "torch_red06.bmp",
    "torch_red07.bmp",
    "torch_red08.bmp",
    "torch_red09.bmp",
    "torch_red10.bmp",
    "torch_red11.bmp",
    "torch_red12.bmp",
    "torch_red13.bmp",
];
/// An orb's inner quad: `share` its side of the outer quad's, `down` and `left` where it sits from the outer quad's
/// middle, shares of the outer quad's side
struct Core {
    texture: &'static str,
    share: f32,
    down: f32,
    left: f32,
}

/// Frames a second of a flame
const FLAME_FPS: f32 = 15.0;

impl SpotLook {
    /// The texture at `frame` (60 a second): a flame plays its frames, the others hold one
    pub fn texture(self, frame: f32) -> &'static str {
        let flame = |frames: &[&'static str]| {
            frames[(frame * FLAME_FPS / FRAMES_PER_SECOND) as usize % frames.len()]
        };
        match self {
            Self::Glow => "thunder_center.bmp",
            Self::Spark => "spark.bmp",
            Self::Soft => "glow11.bmp",
            Self::GreenFlame => flame(&GREEN_FLAME),
            Self::RedFlame => flame(&RED_FLAME),
            Self::WaterOrb => "waterorb.bmp",
            Self::StoneOrb => "stoneorb.bmp",
            Self::FireOrb => "fireorb.bmp",
            Self::LightningOrb => "lightningorb.bmp",
            Self::FireCharm => own_textures::CHARM_FIRE,
            Self::WaterCharm => own_textures::CHARM_WATER,
            Self::EarthCharm => own_textures::CHARM_EARTH,
            Self::WindCharm => own_textures::CHARM_WIND,
        }
    }

    /// A sprite (its own colours over the ground, no core) or a light added to what is behind it
    fn sprite(self) -> bool {
        matches!(
            self,
            Self::WaterOrb
                | Self::StoneOrb
                | Self::FireOrb
                | Self::LightningOrb
                | Self::FireCharm
                | Self::WaterCharm
                | Self::EarthCharm
                | Self::WindCharm
        )
    }

    /// The inner quad. A flame's is a small soft glow at its base: 45% across and 79% down kRO's torch frames (their
    /// base measured, the height set by eye); the others repeat their own at half, in the middle.
    fn core(self, frame: f32) -> Core {
        match self {
            Self::GreenFlame | Self::RedFlame => Core {
                texture: Self::Soft.texture(frame),
                share: 0.3,
                down: 0.29,
                left: 0.05,
            },
            _ => Core {
                texture: self.texture(frame),
                share: 0.5,
                down: 0.0,
                left: 0.0,
            },
        }
    }

    /// What a halo takes from `outer`: a flame's own hue, which a white `outer` leaves to the texture
    fn halo(self, outer: [f32; 3]) -> [f32; 3] {
        let hue = match self {
            Self::GreenFlame => [0.3, 1.0, 0.3],
            Self::RedFlame => [1.0, 0.3, 0.25],
            _ => [1.0; 3],
        };
        [outer[0] * hue[0], outer[1] * hue[1], outer[2] * hue[2]]
    }
}

/// A texture for the aura underfoot
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundLook {
    /// A band around the feet
    Ring,
    /// A filled disc fading outward
    Disc,
}

impl GroundLook {
    fn texture(self) -> &'static str {
        match self {
            Self::Ring => "ring_white.tga",
            Self::Disc => "alpha_down.tga",
        }
    }
}

pub const TEXTURES: &[&str] = &[
    own_textures::CHARM_FIRE,
    own_textures::CHARM_WATER,
    own_textures::CHARM_EARTH,
    own_textures::CHARM_WIND,
    "waterorb.bmp",
    "stoneorb.bmp",
    "fireorb.bmp",
    "lightningorb.bmp",
    "thunder_center.bmp",
    "spark.bmp",
    "glow11.bmp",
    "ring_white.tga",
    "alpha_down.tga",
];

/// Orbs circling the body. `count` None takes the counter's (one orb a shield, a sphere, a soul), up to `max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbLayer {
    pub count: Option<u8>,
    pub max: u8,
    /// How much each count past `max` swells the orbs (a share of `size`) and lights a halo round them
    pub swell: f32,
    pub radius: f32,
    /// Above the feet
    pub height: f32,
    /// 1 turns as a Monk's spheres do; 0 holds them in place
    pub speed: f32,
    /// Degrees a second each orb turns about its own middle (counter-clockwise on the screen; negative, the other way)
    pub spin: f32,
    /// Half the outer quad's side
    pub size: f32,
    pub wobble: f32,
    /// Spread evenly around the circle; otherwise 72 degrees apart, as kRO lays a Monk's five
    pub even: bool,
    /// A wheel standing at the body's back, turning with it, instead of a ring round it on the ground's plane
    pub upright: bool,
    /// How far behind the body the circle's middle sits (away from the camera while the facing is unknown)
    pub back: f32,
    /// How much darker an upright wheel's orbs go toward its bottom: none at the top, that share at the bottom
    pub shade: f32,
    /// How much smaller they go toward the bottom, the same way
    pub shrink: f32,
    pub look: SpotLook,
    pub inner: [f32; 3],
    pub outer: [f32; 3],
    pub intensity: f32,
    /// Stacked, as kRO lays its souls: the newest `max` counts are drawn, the count at index i in slot i % max with
    /// the look of tier i / max (the last tier past them), so each count past `max` lifts one orb a tier. Empty: the
    /// layer's own look for every orb
    pub tiers: &'static [OrbTier],
}

/// One tier of a stacked orb layer: what replaces the layer's look, outer colour and size
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbTier {
    pub look: SpotLook,
    pub outer: [f32; 3],
    pub size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundLayer {
    pub look: GroundLook,
    pub radius: f32,
    /// Degrees a second
    pub spin: f32,
    /// How far the radius swells and shrinks, a share of it
    pub breath: f32,
    pub inner: [f32; 3],
    pub outer: [f32; 3],
    pub intensity: f32,
}

/// kRO's outline glow: a copy of the body behind it, grown by `size` pixels, breathing by `breath`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowLayer {
    pub color: [u8; 3],
    pub size: f32,
    pub breath: f32,
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoteLayer {
    /// Motes a second
    pub rate: f32,
    /// How far one rises in its life
    pub rise: f32,
    /// Seconds
    pub life: f32,
    /// How far from the body's middle one starts
    pub spread: f32,
    pub size: f32,
    pub look: SpotLook,
    pub inner: [f32; 3],
    pub outer: [f32; 3],
    pub intensity: f32,
}

impl MoteLayer {
    /// Seconds, never so short a mote has no time to show
    fn life(&self) -> f32 {
        self.life.max(0.05)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyAuraSpec {
    pub orbs: Option<OrbLayer>,
    pub ground: Option<GroundLayer>,
    pub glow: Option<GlowLayer>,
    pub motes: Option<MoteLayer>,
}

const fn spheres(
    outer: [f32; 3],
    inner: [f32; 3],
    radius: f32,
    height: f32,
    size: f32,
    wobble: f32,
) -> OrbLayer {
    OrbLayer {
        count: None,
        max: 5,
        swell: 0.0,
        radius,
        height,
        speed: 1.0,
        spin: 0.0,
        size,
        wobble,
        even: false,
        upright: false,
        back: 0.0,
        shade: 0.0,
        shrink: 0.0,
        look: SpotLook::Glow,
        inner,
        outer,
        intensity: 200.0 / 255.0,
        tiers: &[],
    }
}

/// A Monk's spirit spheres; past five, bigger and lit (the rig at 10 and 15, October 14)
pub const CHOOKGI: OrbLayer = OrbLayer {
    swell: 0.08,
    ..spheres([0.0, 0.0, 1.0], [1.0, 1.0, 0.0], 7.0, 15.0, 2.0, 1.0)
};
/// A Champion's, swelling as a Monk's do
pub const CHOOKGI2: OrbLayer = OrbLayer {
    swell: 0.08,
    ..spheres([0.0, 0.0, 1.0], [1.0, 1.0, 0.0], 5.5, 14.0, 1.3, 0.7)
};
/// A Gunslinger's coins: brown and white, evenly spread, at half speed
pub const CHOOKGI3: OrbLayer = OrbLayer {
    speed: 0.5,
    even: true,
    ..spheres(
        [68.0 / 255.0, 42.0 / 255.0, 30.0 / 255.0],
        [1.0, 1.0, 1.0],
        7.0,
        15.0,
        1.0,
        0.5,
    )
};
/// A Rune Knight's Millennium Shields: a white core in a violet glow at chest height, spread evenly (the rig's frame,
/// October 14)
pub const MILLENNIUM_SHIELD: OrbLayer = OrbLayer {
    even: true,
    ..spheres([0.35, 0.2, 1.0], [1.0, 1.0, 1.0], 7.0, 11.0, 1.6, 0.5)
};

const fn soul(look: SpotLook, outer: [f32; 3], size: f32) -> OrbTier {
    OrbTier { look, outer, size }
}

/// A Soul Reaper's souls: five at most in a wide ring at the chest, swelling past five; each count past five lifts
/// one soul a tier, as kRO's exe does (its souls at 0x95c380: 13 souls are three of the third tier and two of the
/// second, the rig October 15). Our look per tier: green glints, green flames, red, bigger red (the user's, October
/// 14 and 15)
pub const SOUL_ENERGY: OrbLayer = OrbLayer {
    even: true,
    look: SpotLook::Spark,
    swell: 0.04,
    tiers: &[
        soul(SpotLook::Spark, [0.3, 1.0, 0.3], 1.2),
        soul(SpotLook::GreenFlame, [1.0, 1.0, 1.0], 2.2),
        soul(SpotLook::RedFlame, [1.0, 1.0, 1.0], 2.2),
        soul(SpotLook::RedFlame, [1.0, 1.0, 1.0], 3.0),
    ],
    ..spheres([0.3, 1.0, 0.3], [1.0, 1.0, 1.0], 9.0, 11.0, 1.2, 0.5)
};

/// A Kagerou's or Oboro's charms: one sprite a charm, all ten evenly on a wheel at the body's back that holds still
/// while each orb swirls (the user's, October 14; kRO lays them flat round the head and chest, radius 7.3 at 12,
/// turning)
pub const fn charms(look: SpotLook) -> OrbLayer {
    OrbLayer {
        max: 10,
        even: true,
        upright: true,
        speed: 0.0,
        spin: 120.0,
        back: 3.0,
        shade: 0.5,
        shrink: 0.25,
        look,
        intensity: 1.0,
        ..spheres([1.0; 3], [1.0; 3], 6.5, 10.0, 1.6, 0.3)
    }
}

pub const fn orbs_only(orbs: OrbLayer) -> BodyAuraSpec {
    BodyAuraSpec {
        orbs: Some(orbs),
        ground: None,
        glow: None,
        motes: None,
    }
}

struct Orb {
    orbit_angle: f32,
    /// Its look on a stacked layer
    tier: Option<OrbTier>,
    /// Where its own turn starts, degrees
    phase: f32,
    jit_x: f32,
    jit_y: f32,
    jit_z: f32,
    pulse: f32,
}

struct Mote {
    offset: [f32; 2],
    start: f32,
    age: f32,
}

struct Rng(u32);
impl Rng {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
    fn deg(&mut self) -> f32 {
        (self.next_u32() % 360) as f32
    }
    fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

pub struct BodyAuraEffect {
    world_pos: [f32; 3],
    spec: BodyAuraSpec,
    orbs: Vec<Orb>,
    /// The count past the orbs drawn, which swells them
    extra: u8,
    motes: Vec<Mote>,
    mote_due: f32,
    age_frames: f32,
    /// The unit's facing (`facing_degrees` in radians), for a wheel at its back
    yaw: Option<f32>,
    rng: Rng,
}

impl BodyAuraEffect {
    /// `counter` is the count the server sent (shields, spheres, souls); 1 for a status
    pub fn new(world_pos: [f32; 3], spec: BodyAuraSpec, counter: u8) -> Self {
        let seed = (world_pos[0] * 73.0 + world_pos[2] * 131.0) as i64 as u32 ^ 0x9E37_79B9;
        let mut rng = Rng(seed | 1);
        let wanted = spec.orbs.map_or(0, |layer| layer.count.unwrap_or(counter));
        let orbs: Vec<Orb> = spec
            .orbs
            .map(|layer| {
                let max = layer.max.clamp(1, MAX_ORBS) as usize;
                let count = (wanted as usize).clamp(1, max);
                // a stacked layer draws the newest counts, each in its own slot
                let first = if layer.tiers.is_empty() {
                    0
                } else {
                    (wanted as usize).saturating_sub(max)
                };
                (first..first + count)
                    .map(|i| {
                        let slot = i % max;
                        let tier = (i / max).min(layer.tiers.len().saturating_sub(1));
                        Orb {
                            orbit_angle: if layer.even {
                                slot as f32 * 360.0 / count as f32
                            } else {
                                slot as f32 * 72.0
                            },
                            tier: layer.tiers.get(tier).copied(),
                            phase: rng.deg(),
                            jit_x: rng.deg(),
                            jit_y: rng.deg(),
                            jit_z: rng.deg(),
                            pulse: rng.deg(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let extra = wanted.saturating_sub(orbs.len() as u8);
        Self {
            world_pos,
            spec,
            orbs,
            extra,
            motes: Vec::new(),
            mote_due: 0.0,
            age_frames: 0.0,
            yaw: None,
            rng,
        }
    }

    fn update_motes(&mut self, layer: MoteLayer, delta: f32) {
        for mote in &mut self.motes {
            mote.age += delta;
        }
        let life = layer.life();
        self.motes.retain(|m| m.age < life);
        self.mote_due += layer.rate.max(0.0) * delta;
        while self.mote_due >= 1.0 {
            self.mote_due -= 1.0;
            if self.motes.len() >= MAX_MOTES {
                continue;
            }
            let (angle, reach) = (
                self.rng.unit() * std::f32::consts::TAU,
                self.rng.unit().sqrt() * layer.spread,
            );
            self.motes.push(Mote {
                offset: [reach * angle.cos(), reach * angle.sin()],
                start: self.rng.unit() * MOTE_BODY_HEIGHT,
                age: 0.0,
            });
        }
    }

    fn draw_orbs(&self, layer: &OrbLayer, camera: &CameraView, out: &mut EffectDrawList) {
        let [cx, cy, cz] = self.world_pos;
        let ring = Ring::new(layer, camera, self.yaw);
        let (shade, shrink) = (layer.shade.clamp(0.0, 1.0), layer.shrink.clamp(0.0, 1.0));
        let swell = layer.swell * self.extra as f32;
        let alpha = self.fade_to(layer.intensity);
        for orb in &self.orbs {
            let (look, outer, size) = orb.tier.map_or((layer.look, layer.outer, layer.size), |t| {
                (t.look, t.outer, t.size)
            });
            let size = size * (1.0 + swell);
            let [r, g, b] = look.halo(outer);
            let halo = [r, g, b, alpha * swell.min(0.8)];
            let angle = orb.orbit_angle.to_radians();
            // how far down an upright wheel the orb is, 0 at the top to 1 at the bottom
            let low = if layer.upright {
                (1.0 - ring.rise(angle)) / 2.0
            } else {
                0.0
            };
            let light = 1.0 - shade * low;
            let size = size * (1.0 - shrink * low);
            let [rx, ry, rz] = ring.at(angle);
            let pos = [
                cx + rx + layer.wobble * orb.jit_x.to_radians().sin(),
                cy - layer.height + ry + layer.wobble * orb.jit_y.to_radians().sin(),
                cz + rz + layer.wobble * orb.jit_z.to_radians().sin(),
            ];
            let size = size + orb.pulse.to_radians().sin() * size * 0.05;
            let depth_pos = [pos[0], cy, pos[2]];
            if swell > 0.0 {
                let side = size * HALO_SHARE * SQRT2;
                Quad::new(pos, depth_pos, side, SpotLook::Soft.texture(0.0), halo).push(out);
            }
            // each orb from its own angle, so they do not turn in step
            let turn = (orb.phase + layer.spin * self.age_frames / FRAMES_PER_SECOND).to_radians();
            let look = (look, self.age_frames, turn);
            let (inner, outer) = (layer.inner.map(|c| c * light), outer.map(|c| c * light));
            spot(out, pos, depth_pos, size, look, inner, outer, alpha);
        }
    }

    fn draw_ground(&self, layer: &GroundLayer, out: &mut EffectDrawList) {
        let secs = self.age_frames / FRAMES_PER_SECOND;
        let breath = 1.0 + layer.breath * (secs / GROUND_BREATH_SECS * std::f32::consts::TAU).sin();
        let radius = layer.radius * breath;
        let alpha = self.fade_to(layer.intensity);
        let spin = (layer.spin * secs).to_radians();
        let ring = layer.look == GroundLook::Ring;
        for (radius, color) in [(radius, layer.outer), (radius * 0.5, layer.inner)] {
            out.push(EffectPrimitiveDraw::GroundDisc {
                center: self.world_pos,
                radius,
                thickness: if ring { radius * 0.35 } else { radius },
                rotation: spin,
                arc_angle_deg: 360.0,
                uv_repeat: 1.0,
                texture: layer.look.texture(),
                color: [color[0], color[1], color[2], alpha],
                blend: BlendKind::Additive,
                no_depth: false,
                tilt_rad: 0.0,
                spin_rad: 0.0,
            });
        }
    }

    fn draw_motes(&self, layer: &MoteLayer, out: &mut EffectDrawList) {
        let [cx, cy, cz] = self.world_pos;
        let life = layer.life();
        for mote in &self.motes {
            let t = mote.age / life;
            let pos = [
                cx + mote.offset[0],
                cy - mote.start - layer.rise * t,
                cz + mote.offset[1],
            ];
            let alpha = self.fade_to(layer.intensity) * (t * std::f32::consts::PI).sin();
            let look = (layer.look, self.age_frames, 0.0);
            spot(
                out,
                pos,
                [pos[0], cy, pos[2]],
                layer.size,
                look,
                layer.inner,
                layer.outer,
                alpha,
            );
        }
    }

    fn fade_to(&self, intensity: f32) -> f32 {
        (self.age_frames / FADE_IN_FRAMES).min(1.0) * intensity.clamp(0.0, 1.0)
    }
}

/// The circle orbs ride: flat round the body, or upright at its back
struct Ring {
    radius: f32,
    /// Along the circle at angle 0 and at a quarter turn
    across: [f32; 3],
    along: [f32; 3],
    /// Where its middle sits from above the feet
    back: [f32; 3],
}

impl Ring {
    fn new(layer: &OrbLayer, camera: &CameraView, yaw: Option<f32>) -> Self {
        // the body's back: facing f looks along (sin f, -cos f) on the ground (direction 0, north, is +z); away from
        // the camera while the facing is unknown
        let away = [
            camera.target[0] - camera.eye[0],
            0.0,
            camera.target[2] - camera.eye[2],
        ];
        let back = yaw
            .map(|f| [-f.sin(), 0.0, f.cos()])
            .or_else(|| unit(away))
            .unwrap_or([0.0, 0.0, -1.0]);
        let (across, along) = if layer.upright {
            // in the plane of the back: across the shoulders and up (-y)
            ([-back[2], 0.0, back[0]], [0.0, -1.0, 0.0])
        } else {
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0])
        };
        Self {
            radius: layer.radius,
            across,
            along,
            back: back.map(|a| a * layer.back),
        }
    }

    /// How high on the circle the point at `angle` is, from -1 (the bottom) to 1 (the top); 0 on a flat ring
    fn rise(&self, angle: f32) -> f32 {
        let (s, c) = angle.sin_cos();
        -(self.across[1] * c + self.along[1] * s)
    }

    fn at(&self, angle: f32) -> [f32; 3] {
        let (s, c) = angle.sin_cos();
        std::array::from_fn(|i| {
            self.back[i] + self.radius * (self.across[i] * c + self.along[i] * s)
        })
    }
}

fn norm(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn unit(a: [f32; 3]) -> Option<[f32; 3]> {
    let n = norm(a);
    (n > 1e-6).then(|| a.map(|c| c / n))
}

/// An orb or a mote: the outer colour on a quad of side `2 * size`, the inner on its core; `look` with the effect's
/// age in frames and the turn of the orb about its middle
#[allow(clippy::too_many_arguments)]
fn spot(
    out: &mut EffectDrawList,
    pos: [f32; 3],
    depth_pos: [f32; 3],
    size: f32,
    (look, frame, turn): (SpotLook, f32, f32),
    [ir, ig, ib]: [f32; 3],
    [or, og, ob]: [f32; 3],
    alpha: f32,
) {
    let side = size * SQRT2;
    let outer = Quad {
        turn,
        ..Quad::new(
            pos,
            depth_pos,
            side,
            look.texture(frame),
            [or, og, ob, alpha],
        )
    };
    if look.sprite() {
        return Quad {
            blend: BlendKind::Alpha,
            ..outer
        }
        .push(out);
    }
    outer.push(out);
    let core = look.core(frame);
    let core_pos = [pos[0], pos[1] + side * core.down, pos[2]];
    let inner = [ir, ig, ib, alpha];
    Quad {
        // left by sliding the texture within its quad, which stays square to the screen whatever the camera's turn
        slide: core.left / core.share,
        ..Quad::new(core_pos, depth_pos, side * core.share, core.texture, inner)
    }
    .push(out);
}

/// A square billboard `side` across, its texture slid `slide` of a side to the left and turned `turn` radians
/// (counter-clockwise on the screen)
struct Quad {
    pos: [f32; 3],
    depth_pos: [f32; 3],
    side: f32,
    slide: f32,
    turn: f32,
    texture: &'static str,
    color: [f32; 4],
    blend: BlendKind,
}

impl Quad {
    /// Additive, unslid, unturned
    fn new(
        pos: [f32; 3],
        depth_pos: [f32; 3],
        side: f32,
        texture: &'static str,
        color: [f32; 4],
    ) -> Self {
        Self {
            pos,
            depth_pos,
            side,
            slide: 0.0,
            turn: 0.0,
            texture,
            color,
            blend: BlendKind::Additive,
        }
    }

    fn push(self, out: &mut EffectDrawList) {
        let slide = self.slide;
        out.push(EffectPrimitiveDraw::BillboardDepthAnchored {
            pos: self.pos,
            depth_pos: self.depth_pos,
            size: [self.side, self.side],
            uv: [
                [slide, 0.0],
                [1.0 + slide, 0.0],
                [slide, 1.0],
                [1.0 + slide, 1.0],
            ],
            rotation: self.turn,
            texture: self.texture,
            color: self.color,
            blend: self.blend,
        });
    }
}

impl Effect for BodyAuraEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        let frames = ctx.delta * FRAMES_PER_SECOND;
        self.age_frames += frames;
        self.yaw = ctx.caster_yaw;
        if let Some(layer) = self.spec.orbs {
            let step = ORBIT_DEG_PER_FRAME * layer.speed * frames;
            for orb in &mut self.orbs {
                orb.orbit_angle = (orb.orbit_angle + step) % 360.0;
                orb.jit_x = (orb.jit_x + frames) % 360.0;
                orb.jit_y = (orb.jit_y + frames) % 360.0;
                orb.jit_z = (orb.jit_z + frames) % 360.0;
                orb.pulse = (orb.pulse + 7.0 * frames) % 360.0;
            }
        }
        if let Some(layer) = self.spec.motes {
            self.update_motes(layer, ctx.delta);
        }
        EffectStatus::Running
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.world_pos = pos;
    }

    fn collect_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        if self.age_frames <= 0.0 {
            return;
        }
        if let Some(layer) = &self.spec.ground {
            self.draw_ground(layer, out);
        }
        if let Some(layer) = &self.spec.orbs {
            self.draw_orbs(layer, &ctx.camera, out);
        }
        if let Some(layer) = &self.spec.motes {
            self.draw_motes(layer, out);
        }
    }

    fn body_copies(&self) -> Option<Vec<BodyCopy>> {
        let layer = self.spec.glow?;
        Some(vec![BodyCopy {
            offset_px: [0.0, 0.0],
            margin_px: 0.0,
            scale: [1.0, 1.0],
            tint: layer.color,
            alpha: self.fade_to(layer.intensity),
            additive: true,
            behind: true,
            body_layers_only: true,
            glow: Some(BodyGlow {
                age_frames: self.age_frames,
                base_px: layer.size,
                amp_px: layer.breath,
            }),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_ctx() -> EffectRenderCtx {
        EffectRenderCtx {
            camera: Default::default(),
            screen_w: 800.0,
            screen_h: 600.0,
            elapsed: 0.0,
        }
    }

    fn tick(e: &mut BodyAuraEffect, frames: u32) {
        for _ in 0..frames {
            e.update(&EffectUpdateCtx {
                delta: 1.0 / FRAMES_PER_SECOND,
                camera_target: None,
                caster_yaw: None,
            });
        }
    }

    fn draws(e: &BodyAuraEffect) -> Vec<EffectPrimitiveDraw> {
        let mut l = EffectDrawList::new();
        e.collect_draws(&mut l, &render_ctx());
        l.primitives
    }

    fn billboard(d: &EffectPrimitiveDraw) -> ([f32; 3], [f32; 4], f32, &'static str) {
        match d {
            EffectPrimitiveDraw::BillboardDepthAnchored {
                pos,
                color,
                size,
                texture,
                ..
            } => (*pos, *color, size[0], texture),
            other => panic!("not a billboard: {other:?}"),
        }
    }

    fn spheres(count: u8) -> BodyAuraEffect {
        BodyAuraEffect::new([0.0; 3], orbs_only(CHOOKGI), count)
    }

    #[test]
    fn two_quads_an_orb_one_orb_a_count() {
        for count in [1u8, 3, 5] {
            let mut e = spheres(count);
            tick(&mut e, 5);
            assert_eq!(draws(&e).len(), count as usize * 2);
        }
        let mut clamped = spheres(99);
        tick(&mut clamped, 5);
        assert_eq!(
            draws(&clamped).len(),
            5 * 3,
            "a Monk's spheres stop at five, haloed past it"
        );
        let souls = OrbLayer {
            max: 99,
            swell: 0.0,
            ..CHOOKGI
        };
        let mut all = BodyAuraEffect::new([0.0; 3], orbs_only(souls), 99);
        tick(&mut all, 5);
        assert_eq!(draws(&all).len(), MAX_ORBS as usize * 2);
    }

    #[test]
    fn a_fixed_count_ignores_the_counter() {
        let layer = OrbLayer {
            count: Some(3),
            ..CHOOKGI
        };
        let mut e = BodyAuraEffect::new([0.0; 3], orbs_only(layer), 9);
        tick(&mut e, 5);
        assert_eq!(draws(&e).len(), 3 * 2);
    }

    #[test]
    fn orbs_orbit_above_the_feet() {
        let mut e = spheres(5);
        tick(&mut e, 3);
        let a = billboard(&draws(&e)[0]).0;
        tick(&mut e, 60);
        let b = billboard(&draws(&e)[0]).0;
        assert!(
            (a[0] - b[0]).abs() + (a[2] - b[2]).abs() > 1e-3,
            "orb orbits"
        );
        assert!(b[1] < 0.0, "orb rides above the caster's feet");
    }

    #[test]
    fn orbs_fade_in_to_the_spheres_peak() {
        let mut e = spheres(5);
        tick(&mut e, 3);
        let early = billboard(&draws(&e)[0]).1[3];
        tick(&mut e, 400);
        let late = billboard(&draws(&e)[0]).1[3];
        assert!(late > early);
        assert!(
            (late - 200.0 / 255.0).abs() < 1e-4,
            "a Monk's spheres peak at 200/255, got {late}"
        );
    }

    #[test]
    fn the_outer_quad_is_larger_and_its_own_colour() {
        let mut e = spheres(5);
        tick(&mut e, 5);
        let d = draws(&e);
        let (outer, inner) = (billboard(&d[0]), billboard(&d[1]));
        assert!(outer.2 > inner.2);
        assert_ne!(outer.1, inner.1);
    }

    #[test]
    fn the_sphere_variants_spread_and_turn_as_kro_lays_them() {
        assert!(CHOOKGI2.radius < CHOOKGI.radius);
        assert!(CHOOKGI3.even && CHOOKGI3.speed == 0.5 && !CHOOKGI.even);
        let even = BodyAuraEffect::new([0.0; 3], orbs_only(CHOOKGI3), 3);
        assert!((even.orbs[1].orbit_angle - 120.0).abs() < 1e-3);
        let fixed = spheres(3);
        assert!((fixed.orbs[1].orbit_angle - 72.0).abs() < 1e-3);
    }

    #[test]
    fn orbs_follow_the_unit() {
        let mut e = spheres(5);
        tick(&mut e, 5);
        e.set_position([40.0, 0.0, -12.0]);
        let p = billboard(&draws(&e)[0]).0;
        assert!((p[0] - 40.0).abs() < 8.0 && (p[2] + 12.0).abs() < 8.0);
    }

    const GROUND: GroundLayer = GroundLayer {
        look: GroundLook::Ring,
        radius: 6.0,
        spin: 30.0,
        breath: 0.1,
        inner: [1.0, 1.0, 1.0],
        outer: [0.4, 0.2, 1.0],
        intensity: 0.5,
    };
    const GLOW: GlowLayer = GlowLayer {
        color: [90, 50, 255],
        size: 5.0,
        breath: 1.5,
        intensity: 0.7,
    };
    const MOTES: MoteLayer = MoteLayer {
        rate: 10.0,
        rise: 20.0,
        life: 1.0,
        spread: 5.0,
        size: 1.0,
        look: SpotLook::Spark,
        inner: [1.0, 1.0, 1.0],
        outer: [0.4, 1.0, 0.4],
        intensity: 0.8,
    };

    #[test]
    fn the_ground_aura_lies_at_the_feet_in_its_colours_and_spins() {
        let spec = BodyAuraSpec {
            ground: Some(GROUND),
            ..Default::default()
        };
        let mut e = BodyAuraEffect::new([3.0, 1.0, 4.0], spec, 1);
        tick(&mut e, 600);
        let discs: Vec<_> = draws(&e)
            .into_iter()
            .filter_map(|d| match d {
                EffectPrimitiveDraw::GroundDisc {
                    center,
                    color,
                    rotation,
                    radius,
                    ..
                } => Some((center, color, rotation, radius)),
                _ => None,
            })
            .collect();
        assert_eq!(discs.len(), 2);
        assert_eq!(discs[0].0, [3.0, 1.0, 4.0]);
        assert_eq!(&discs[0].1[..3], &GROUND.outer);
        assert!(
            (discs[0].1[3] - 0.5).abs() < 1e-4,
            "intensity scales the alpha"
        );
        assert!(discs[0].2 > 0.0, "spins");
        assert!(discs[1].3 < discs[0].3, "the inner colour inside");
    }

    #[test]
    fn the_glow_is_a_breathing_copy_behind_the_body() {
        let spec = BodyAuraSpec {
            glow: Some(GLOW),
            ..Default::default()
        };
        let mut e = BodyAuraEffect::new([0.0; 3], spec, 1);
        assert!(draws(&e).is_empty());
        tick(&mut e, 600);
        let copies = e.body_copies().expect("a copy");
        let [copy] = copies.as_slice() else { panic!() };
        assert!(copy.behind && copy.additive && copy.body_layers_only);
        assert_eq!(copy.tint, GLOW.color);
        assert!((copy.alpha - 0.7).abs() < 1e-4);
        let glow = copy.glow.expect("breathing");
        assert_eq!((glow.base_px, glow.amp_px), (5.0, 1.5));
        assert!(
            BodyAuraEffect::new([0.0; 3], orbs_only(CHOOKGI), 1)
                .body_copies()
                .is_none()
        );
    }

    #[test]
    fn motes_are_born_at_the_rate_rise_and_die_at_their_life() {
        let spec = BodyAuraSpec {
            motes: Some(MOTES),
            ..Default::default()
        };
        let mut e = BodyAuraEffect::new([0.0; 3], spec, 1);
        tick(&mut e, 30);
        assert_eq!(e.motes.len(), 5, "10 a second for half a second");
        let first = e.motes[0].start;
        let y = billboard(&draws(&e)[0]).0[1];
        assert!(y < -first, "risen above where it started");
        tick(&mut e, 600);
        assert!(
            e.motes.len() <= 11,
            "a second's life at 10 a second: {}",
            e.motes.len()
        );
        assert!(e.motes.iter().all(|m| m.age < 1.0));
        for d in draws(&e) {
            let (pos, _, _, texture) = billboard(&d);
            assert!(
                texture == "spark.bmp" || texture == "glow11.bmp",
                "{texture}"
            );
            assert!(pos[0].hypot(pos[2]) <= MOTES.spread + 1e-3);
        }
    }

    /// As kRO's exe lays them (the rig, October 15): 4 souls are four glints a quarter turn apart; 13 are three of
    /// the third tier and two of the second, each in the slot its count fills; 20 all of the last, swollen as before
    #[test]
    fn souls_past_five_lift_one_orb_a_tier() {
        let orbs = |count| {
            let e = BodyAuraEffect::new([0.0; 3], orbs_only(SOUL_ENERGY), count);
            let orbs: Vec<_> = e
                .orbs
                .iter()
                .map(|o| (o.orbit_angle, o.tier.unwrap()))
                .collect();
            (orbs, e.extra)
        };
        let &[glint, green, red, big_red] = SOUL_ENERGY.tiers else {
            panic!("four tiers");
        };
        let (four, _) = orbs(4);
        assert_eq!(four, [0.0, 90.0, 180.0, 270.0].map(|a| (a, glint)).to_vec());
        let (thirteen, _) = orbs(13);
        assert_eq!(
            thirteen,
            vec![
                (216.0, green),
                (288.0, green),
                (0.0, red),
                (72.0, red),
                (144.0, red)
            ]
        );
        let (twenty, extra) = orbs(20);
        assert!(twenty.iter().all(|&(_, t)| t == big_red));
        assert_eq!(extra, 15, "past five they still swell");
    }

    #[test]
    fn counts_past_the_most_drawn_swell_the_orbs_and_light_a_halo() {
        let at = |count| {
            let mut e = spheres(count);
            tick(&mut e, 5);
            draws(&e)
        };
        let (five, fifteen) = (at(5), at(15));
        assert_eq!(five.len(), 5 * 2, "no halo up to five");
        assert_eq!(fifteen.len(), 5 * 3, "a halo an orb past five");
        let outer = |d: &[EffectPrimitiveDraw], i| billboard(&d[i]).2;
        assert!(
            outer(&fifteen, 1) > outer(&five, 0) * 1.7,
            "ten past five: 80% bigger"
        );
        assert_eq!(billboard(&fifteen[0]).3, "glow11.bmp");
        assert!(at(10).len() == 15 && outer(&at(10), 1) < outer(&fifteen, 1));
    }

    #[test]
    fn a_charm_is_one_sprite_drawn_as_it_is() {
        let mut e = BodyAuraEffect::new([0.0; 3], orbs_only(charms(SpotLook::FireOrb)), 10);
        tick(&mut e, 300);
        let d = draws(&e);
        assert_eq!(d.len(), 10, "one quad a charm, all ten");
        let EffectPrimitiveDraw::BillboardDepthAnchored {
            texture,
            blend,
            color,
            ..
        } = &d[0]
        else {
            panic!()
        };
        assert_eq!(
            (*texture, *blend, color[3]),
            ("fireorb.bmp", BlendKind::Alpha, 1.0)
        );
    }

    #[test]
    fn an_upright_ring_stands_on_the_bodys_back_and_turns_with_it() {
        let layer = OrbLayer {
            wobble: 0.0,
            ..charms(SpotLook::FireOrb)
        };
        let mut e = BodyAuraEffect::new([0.0; 3], orbs_only(layer), 10);
        // facing f looks along (sin f, -cos f): 180 is north (+z), its back -z; 270 is west (-x), its back +x
        for (facing, back) in [
            (180.0f32, [0.0, 0.0, -1.0]),
            (0.0, [0.0, 0.0, 1.0]),
            (270.0, [1.0, 0.0, 0.0]),
        ] {
            e.update(&EffectUpdateCtx {
                delta: 1.0 / FRAMES_PER_SECOND,
                camera_target: None,
                caster_yaw: Some(facing.to_radians()),
            });
            let middle = [back[0] * layer.back, -layer.height, back[2] * layer.back];
            for d in draws(&e) {
                let EffectPrimitiveDraw::BillboardDepthAnchored { pos, depth_pos, .. } = d else {
                    panic!()
                };
                let off: [f32; 3] = std::array::from_fn(|i| pos[i] - middle[i]);
                let along_back = off[0] * back[0] + off[2] * back[2];
                assert!(
                    along_back.abs() < 1e-3,
                    "in the plane of the back, facing {facing}: {off:?}"
                );
                assert!(
                    (norm(off) - layer.radius).abs() < 1e-3,
                    "on the circle: {off:?}"
                );
                assert_eq!(
                    depth_pos,
                    [pos[0], 0.0, pos[2]],
                    "each orb sorts where it is"
                );
            }
        }
    }

    #[test]
    fn an_upright_wheel_darkens_and_shrinks_toward_its_bottom() {
        let layer = OrbLayer {
            wobble: 0.0,
            ..charms(SpotLook::FireOrb)
        };
        let mut e = BodyAuraEffect::new([0.0; 3], orbs_only(layer), 4);
        e.age_frames = 400.0;
        let spots: Vec<_> = draws(&e).iter().map(billboard).collect();
        let top = spots
            .iter()
            .min_by(|a, b| a.0[1].total_cmp(&b.0[1]))
            .unwrap();
        let bottom = spots
            .iter()
            .max_by(|a, b| a.0[1].total_cmp(&b.0[1]))
            .unwrap();
        assert_eq!(top.1, [1.0, 1.0, 1.0, 1.0], "full colour at the top");
        assert_eq!(
            bottom.1,
            [0.5, 0.5, 0.5, 1.0],
            "half as bright at the bottom, as solid"
        );
        let shrunk = bottom.2 / top.2;
        assert!(
            (0.68..0.82).contains(&shrunk),
            "a quarter smaller at the bottom, but for the pulse: {shrunk}"
        );
        let flat = BodyAuraEffect::new(
            [0.0; 3],
            orbs_only(OrbLayer {
                shade: 0.5,
                ..CHOOKGI
            }),
            5,
        );
        let colours: Vec<_> = draws(&flat)
            .iter()
            .step_by(2)
            .map(|d| billboard(d).1)
            .collect();
        assert!(
            colours.windows(2).all(|w| w[0] == w[1]),
            "a flat ring is not shaded"
        );
    }

    #[test]
    fn charms_hold_still_on_their_wheel_and_swirl_in_place() {
        let layer = OrbLayer {
            wobble: 0.0,
            ..charms(SpotLook::FireCharm)
        };
        let mut e = BodyAuraEffect::new([0.0; 3], orbs_only(layer), 10);
        let at = |e: &BodyAuraEffect| -> Vec<([f32; 3], f32)> {
            draws(e)
                .into_iter()
                .map(|d| match d {
                    EffectPrimitiveDraw::BillboardDepthAnchored { pos, rotation, .. } => {
                        (pos, rotation)
                    }
                    other => panic!("{other:?}"),
                })
                .collect()
        };
        tick(&mut e, 1);
        let before = at(&e);
        tick(&mut e, 30);
        let after = at(&e);
        for ((p0, r0), (p1, r1)) in before.iter().zip(&after) {
            let moved = (0..3).map(|i| (p0[i] - p1[i]).abs()).fold(0.0, f32::max);
            assert!(
                moved < 0.2,
                "the wheel holds still (but for the pulse's nothing): {moved}"
            );
            let turned = (r1 - r0).to_degrees();
            assert!(
                (turned - 60.0).abs() < 1e-2,
                "120 degrees a second, half a second: {turned}"
            );
        }
        let starts: Vec<f32> = before.iter().map(|&(_, r)| r).collect();
        assert!(starts.windows(2).any(|w| w[0] != w[1]), "not in step");
    }

    #[test]
    fn a_flame_plays_its_frames_round_a_soft_core() {
        let layer = OrbLayer {
            look: SpotLook::RedFlame,
            ..CHOOKGI
        };
        let mut e = BodyAuraEffect::new([0.0; 3], orbs_only(layer), 1);
        tick(&mut e, 1);
        let d = draws(&e);
        assert_eq!(
            (billboard(&d[0]).3, billboard(&d[1]).3),
            ("torch_red01.bmp", "glow11.bmp")
        );
        assert!(
            billboard(&d[1]).0[1] > billboard(&d[0]).0[1],
            "the core low in the flame"
        );
        tick(&mut e, 4 * 13);
        assert_eq!(
            billboard(&draws(&e)[0]).3,
            "torch_red01.bmp",
            "13 frames at 15 a second, round again"
        );
        tick(&mut e, 4);
        assert_eq!(billboard(&draws(&e)[0]).3, "torch_red02.bmp");
    }

    #[test]
    fn the_layers_draw_together() {
        let spec = BodyAuraSpec {
            orbs: Some(MILLENNIUM_SHIELD),
            ground: Some(GROUND),
            glow: Some(GLOW),
            motes: Some(MOTES),
        };
        let mut e = BodyAuraEffect::new([0.0; 3], spec, 3);
        tick(&mut e, 60);
        let d = draws(&e);
        let discs = d
            .iter()
            .filter(|d| matches!(d, EffectPrimitiveDraw::GroundDisc { .. }))
            .count();
        assert_eq!(discs, 2);
        assert_eq!(d.len() - discs, 3 * 2 + e.motes.len() * 2);
        assert!(e.body_copies().is_some());
    }
}
