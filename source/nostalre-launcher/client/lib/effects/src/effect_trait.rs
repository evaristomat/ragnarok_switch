use super::draw::{EffectDrawList, EffectStatus};

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraView {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
}

impl CameraView {
    /// The camera's turn about the world's Y in degrees, as kRO keeps it (`CView` +0x30): the renderer camera's
    /// yaw (its eye sits at `target + (sin yaw, _, −cos yaw)`)
    pub fn turn_degrees(&self) -> f32 {
        let [ex, _, ez] = self.eye;
        let [tx, _, tz] = self.target;
        (ex - tx).atan2(-(ez - tz)).to_degrees()
    }
}

#[derive(Default, Clone, Copy)]
pub struct EffectUpdateCtx {
    pub delta: f32,
    pub camera_target: Option<[f32; 3]>,
    pub caster_yaw: Option<f32>,
}

pub struct EffectRenderCtx {
    pub camera: CameraView,
    pub screen_w: f32,
    pub screen_h: f32,
    pub elapsed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyTint {
    pub rgb: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraShake {
    pub amplitude: f32,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Afterimage {
    pub tint: [u8; 3],
    pub start_alpha: f32,
    pub fade_per_frame: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyAction {
    pub action_index: usize,
    pub start_frame: usize,
    pub duration_ms: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyVertical {
    pub lift_px: f32,
    pub alpha: f32,
    pub squeeze: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyCopy {
    pub offset_px: [f32; 2],
    pub scale: [f32; 2],
    /// Margin added to every edge of every sprite clip, in screen pixels at a
    /// 400-unit camera distance. Closer cameras widen it proportionally.
    pub margin_px: f32,
    pub tint: [u8; 3],
    pub alpha: f32,
    pub additive: bool,
    pub behind: bool,
    /// Skip the weapon and shield layers, so only the actor itself glows.
    pub body_layers_only: bool,
    /// When set, replaces `margin_px`, and the copy also takes the actor's own
    /// tint and alpha.
    pub glow: Option<BodyGlow>,
}

/// A margin whose sine steps once per drawn body layer, so an actor wearing
/// more headgears pulses faster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyGlow {
    pub age_frames: f32,
    pub base_px: f32,
    pub amp_px: f32,
}

impl BodyGlow {
    pub fn margin_px(&self, body_layers: u8) -> f32 {
        let deg = (self.age_frames * body_layers as f32) % 181.0;
        self.base_px + self.amp_px * deg.to_radians().sin()
    }
}

/// How an effect lights the weapon layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WeaponLight {
    #[default]
    None,
    /// An extra additive draw over the normal one, on every other frame.
    Spark,
    /// Drawn additively instead of normally, held every frame.
    Glow,
    /// An aura drawn around the weapon by the weapon aura shader. nostalRE's own, for the statuses
    /// `custom.skill.weapon_aura` lists; kRO has no such light.
    Aura(WeaponAura),
}

/// A weapon aura: its look, its colours, and the clock it moves with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponAura {
    pub style: WeaponAuraStyle,
    /// The hottest or brightest part, at the weapon
    pub inner: [u8; 3],
    /// Where it fades out, away from the weapon
    pub outer: [u8; 3],
    /// Frames at 60 a second
    pub frame: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponAuraStyle {
    /// Flames rising off the weapon and breaking into tongues
    Flame,
    /// A glow hugging the weapon's outline, breathing a faint ring outward
    Pulse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberRequest {
    pub value: i32,
    pub color: [f32; 3],
}

pub type GroundSampler = std::sync::Arc<dyn Fn(f32, f32) -> f32 + Send + Sync>;

/// What a live keyed effect is told by its owner's packets
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectMessage {
    /// A count: a unit's servants, spheres or coins (0x01d0)
    Count(u8),
    /// A skill spent the servants it holds: one, or all of them (kRO's 0x957960 with the skill's id)
    Spent {
        skill: models::enums::skill_enums::SkillEnum,
        all: bool,
    },
}

/// One STR an effect plays among several (kRO's classes that keep STRs by slot): its file under `texture/effect`
/// (no `.str`), where it sits from the effect's position in the camera's own axes (x right, y down, z away, in world
/// units, as kRO adds a slot's offset in view space), its size, its clock, whether it goes round, and whether it lies
/// under the units (the type kRO makes it with: 0xd files it in its first list, +0x184; 5 in +0x160, over them)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrPiece {
    pub file: &'static str,
    pub view_offset: [f32; 3],
    pub scale: f32,
    pub anim_time: f32,
    pub repeat: bool,
    pub under_sprites: bool,
    /// Where it plays in the world, in place of the effect's owner (a thrown or flying STR's own path)
    pub at: Option<[f32; 3]>,
    /// Its turn on screen about its centre (kRO's STR player +0x3fa4)
    pub turn: StrTurn,
    /// Its layers' alpha, times this (kRO's +0x3fac: a fade the code makes)
    pub alpha: f32,
}

impl StrPiece {
    /// An STR at the effect's own place, unturned, at its size and alpha
    pub fn new(file: &'static str, anim_time: f32, repeat: bool) -> Self {
        Self {
            file,
            view_offset: [0.0; 3],
            scale: 1.0,
            anim_time,
            repeat,
            under_sprites: false,
            at: None,
            turn: StrTurn::None,
            alpha: 1.0,
        }
    }
}

/// How an STR turns on screen about its centre (kRO's STR player +0x3fa4, in degrees, 0 to the right turning
/// clockwise, applied when above 0: the render 0x9831f0)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrTurn {
    None,
    Fixed(f32),
    /// Toward a point in the world as the camera shows it, plus `plus` degrees: kRO's aim, `acos(dx / len)` on the
    /// projected way to the target, 360 less it when the target is higher on screen (0x9cd210)
    Toward {
        point: [f32; 3],
        plus: f32,
    },
}

pub trait Effect: Send {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus;
    fn collect_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx);

    fn set_ground_sampler(&mut self, _sampler: GroundSampler) {}

    fn is_placeholder(&self) -> bool {
        false
    }

    fn set_link_endpoints(&mut self, _caster: [f32; 3], _target: [f32; 3]) {}

    fn set_position(&mut self, _pos: [f32; 3]) {}

    fn str_overlay(&self) -> Option<&'static str> {
        None
    }

    /// The STRs it plays by slot ([`StrPiece`])
    fn str_pieces(&self, _out: &mut Vec<StrPiece>) {}

    /// Whether it ends once its STRs have all played to their files' ends, which only the renderer knows
    /// (`EffectHolder::end_played_strs`)
    fn ends_with_its_strs(&self) -> bool {
        false
    }

    /// What its owner's packets tell it while it lives ([`EffectMessage`])
    fn on_message(&mut self, _message: EffectMessage) {}

    fn weapon_trail(&self) -> bool {
        false
    }

    fn body_tint(&self) -> Option<BodyTint> {
        None
    }

    fn body_additive(&self) -> bool {
        false
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        None
    }

    fn take_number_request(&mut self) -> Option<NumberRequest> {
        None
    }

    fn take_camera_shake(&mut self) -> Option<CameraShake> {
        None
    }

    /// Independent per-edge jitter in screen pixels, ordered top, bottom, left,
    /// right. Unequal edges stretch the body quad as well as move it.
    fn body_edge_jitter(&self) -> Option<[f32; 4]> {
        None
    }

    /// One extra additive draw of the weapon layer, over its normal draw, this
    /// frame.
    fn body_weapon_light(&self) -> WeaponLight {
        WeaponLight::None
    }

    fn body_afterimage(&self) -> Option<Afterimage> {
        None
    }

    fn body_yaw(&self) -> Option<f32> {
        None
    }

    fn body_scale(&self) -> Option<f32> {
        None
    }

    fn take_body_action(&mut self) -> Option<BodyAction> {
        None
    }

    fn body_vertical(&self) -> Option<BodyVertical> {
        None
    }

    fn body_angle(&self) -> Option<f32> {
        None
    }

    fn body_copies(&self) -> Option<Vec<BodyCopy>> {
        None
    }
}
