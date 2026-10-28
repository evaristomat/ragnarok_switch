//! What kRO draws for a unit's spheres when its job has a look of its own (0x01d0's handler 0x805a80, by job): a
//! Dragon Knight's servant weapons (`CServantWeaponEffect`, class 1694), an Abyss Chaser's abyss spheres
//! (`CFromTheAbyssEffect`, 2109) and a Rebellion's or Night Watch's coins (`CRichsCoinEffect`, 1135).
//!
//! The servants and the spheres keep STRs by slot around their owner, placed each update at the owner's position
//! plus a slot offset in the camera's own axes (0x957fe0, 0x9d2910); the servants are made as type 0xd, drawn under
//! the units (kRO's first list, +0x184), the spheres as type 5, over them (+0x160). A slot's STR that does not go
//! round ends when its frames reach its length (`+0xcc` x 60). The lengths are the STR files' own (`max_key / fps`,
//! all at 60 fps). The coins are textures turning round their owner (0x956780, 0x956a40), over the units.
//!
//! These looks are to be redone with nostalRE's own weapon auras (the user, October 17); the ports are their basis.

use models::enums::effect_id::EffectId;
use models::enums::skill_enums::SkillEnum;

use crate::draw::{BILLBOARD_UV, BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect_trait::{Effect, EffectMessage, EffectRenderCtx, EffectUpdateCtx, StrPiece};
use crate::kro_str::Packet;

/// The job ids whose spheres kRO draws by a class of their own (0x805a80): the Dragon Knights (0x109c, 0x10a9,
/// 0x10b8), the Abyss Chaser (0x10a4), the Rebellions and the Night Watch (0x1077, 0x1085, 0x10d2)
pub fn sphere_class(job: u16) -> Option<EffectId> {
    match job {
        4252 | 4265 | 4280 => Some(EffectId::Kro1694),
        4260 => Some(EffectId::Kro2109),
        4215 | 4229 | 4306 => Some(EffectId::Kro1135),
        _ => None,
    }
}

/// What a new count does to a unit's sphere class (0x805a80)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Renewal {
    /// It is told the count
    Keep,
    /// It is taken off and made anew with the count: the coins at every count (0x96b3d0, then 0x96aca0), the abyss
    /// spheres of a hidden or cloaked unit
    Fresh,
    /// It is taken off, the count not taken: the servants of a hidden or cloaked unit (0x96b470)
    Remove,
}

/// `hidden`: [`crate::hidden_or_cloaked`]
pub fn renewal(class: EffectId, hidden: bool) -> Renewal {
    match class {
        EffectId::Kro1694 if hidden => Renewal::Remove,
        EffectId::Kro2109 if hidden => Renewal::Fresh,
        EffectId::Kro1135 => Renewal::Fresh,
        _ => Renewal::Keep,
    }
}

/// The servants a skill spends, and when: Servant Weapon - Sign one at its cast's start (0x0b1a, 0x7ef530),
/// Phantom and Demolition all of them at their no-damage packet (0x8068e0); none else (a count drop takes the rest)
pub fn servants_spent(skill: SkillEnum, packet: Packet) -> Option<EffectMessage> {
    let all = match (skill, packet) {
        (SkillEnum::DkServantWSign, Packet::Cast) => false,
        (SkillEnum::DkServantWPhantom | SkillEnum::DkServantWDemol, Packet::NoDamage) => true,
        _ => return None,
    };
    Some(EffectMessage::Spent { skill, all })
}

/// An STR file and how long it plays
#[derive(Clone, Copy, Debug, PartialEq)]
struct StrFile {
    name: &'static str,
    secs: f32,
}

/// One STR in a slot, from its start
#[derive(Clone, Copy, Debug, PartialEq)]
struct SlotStr {
    file: StrFile,
    age: f32,
    repeat: bool,
}

impl SlotStr {
    fn once(file: StrFile) -> Self {
        Self {
            file,
            age: 0.0,
            repeat: false,
        }
    }

    fn looping(file: StrFile) -> Self {
        Self {
            repeat: true,
            ..Self::once(file)
        }
    }

    fn done(&self) -> bool {
        !self.repeat && self.age >= self.file.secs
    }

    /// One going round keeps its clock within one turn, so a long session loses no precision
    fn advance(&mut self, delta: f32) {
        self.age += delta;
        if self.repeat {
            self.age %= self.file.secs;
        }
    }

    fn piece(&self, view_offset: [f32; 3], scale: f32, under_sprites: bool) -> StrPiece {
        StrPiece {
            view_offset,
            scale,
            under_sprites,
            ..StrPiece::new(self.file.name, self.age, self.repeat)
        }
    }
}

/// Ages a slot's STR and lets it go when it has played through
fn tick(slot: &mut Option<SlotStr>, delta: f32) -> Option<SlotStr> {
    let s = slot.as_mut()?;
    if s.done() {
        return slot.take();
    }
    s.advance(delta);
    None
}

const SERVANT_CAST: StrFile = StrFile {
    name: "servantweapon/servantweapon_cast/servantweapon_cast",
    secs: 37.0 / 60.0,
};
const SERVANT: StrFile = StrFile {
    name: "servantweapon/servantweapon/servantweapon",
    secs: 30.0 / 60.0,
};
const SERVANT_SOUND: &str = "effect\\DK_SERVANTWEAPON.wav";

/// The farewell a servant takes by the skill that spends it (0x957960): Sign's and Phantom's own shots, else the
/// servant shooting off (0x1452 a count drop, 0x1455 Demolition)
fn servant_shot(skill: SkillEnum) -> StrFile {
    match skill {
        SkillEnum::DkServantWSign => StrFile {
            name: "servant_sign/servant_sign_shot/servant_sign_shot",
            secs: 49.0 / 60.0,
        },
        SkillEnum::DkServantWPhantom => StrFile {
            name: "servant_phantom/servant_phantom_shot/servant_phantom_shot",
            secs: 31.0 / 60.0,
        },
        _ => StrFile {
            name: "servantweapon/servantweapon_shoot/servantweapon_shoot",
            secs: 11.0 / 60.0,
        },
    }
}

/// The skill 0x805a80 passes when a count drop takes a servant (0x1452)
const COUNT_DROP: SkillEnum = SkillEnum::DkServantweaponAtk;

/// The five slots' offsets (0x957bf0: +0xc8 on, 12 bytes each): an arc over the knight's shoulders
const SERVANT_SLOTS: [[f32; 3]; 5] = [
    [-7.0, -2.0, 0.0],
    [-10.0, -12.0, 0.0],
    [0.0, -19.0, 0.0],
    [10.0, -12.0, 0.0],
    [7.0, -2.0, 0.0],
];

#[derive(Default)]
struct ServantSlot {
    /// The servant itself, going round (+0x104)
    weapon: Option<SlotStr>,
    /// Its coming (+0x118), the servant once it has played
    cast: Option<SlotStr>,
    /// Its going (+0x12c)
    shot: Option<SlotStr>,
    /// When the servant came to stand, in the order servants did (the one standing longest goes first)
    stood: u32,
}

impl ServantSlot {
    /// A servant stands here, or is coming (0x957b80 counts either)
    fn held(&self) -> bool {
        self.weapon.is_some() || self.cast.is_some()
    }
}

/// A Dragon Knight's servant weapons (`CServantWeaponEffect`)
pub struct ServantWeaponEffect {
    slots: [ServantSlot; 5],
    sound: bool,
    /// The next servant's [`ServantSlot::stood`]
    next_stood: u32,
}

impl ServantWeaponEffect {
    /// Made full, five coming (0x957bf0), then taken to the count 0x01d0 brought, as its handler does
    pub fn new(count: u8) -> Self {
        let mut effect = Self {
            slots: Default::default(),
            sound: false,
            next_stood: 0,
        };
        for slot in &mut effect.slots {
            slot.cast = Some(SlotStr::once(SERVANT_CAST));
        }
        effect.on_message(EffectMessage::Count(count));
        effect
    }

    fn count(&self) -> usize {
        self.slots.iter().filter(|s| s.held()).count()
    }

    /// One more, in the first free slot (0x957560)
    fn add(&mut self) {
        if let Some(slot) = self.slots.iter_mut().find(|s| !s.held()) {
            slot.cast = Some(SlotStr::once(SERVANT_CAST));
        }
    }

    /// One fewer (0x957960), spent by `skill`: the servant that has stood longest, else the first one coming, with
    /// that skill's farewell
    fn remove(&mut self, skill: SkillEnum) {
        let oldest = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.weapon.is_some())
            .min_by_key(|(_, s)| s.stood)
            .map(|(i, _)| i);
        let at = match oldest {
            Some(i) => {
                self.slots[i].weapon = None;
                i
            }
            None => match self.slots.iter().position(|s| s.cast.is_some()) {
                Some(i) => {
                    self.slots[i].cast = None;
                    i
                }
                None => return,
            },
        };
        self.slots[at].shot = Some(SlotStr::once(servant_shot(skill)));
    }
}

impl Effect for ServantWeaponEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        for slot in &mut self.slots {
            if let Some(w) = slot.weapon.as_mut() {
                w.advance(ctx.delta);
            }
            // a coming servant that has played becomes the servant (0x957e30)
            if tick(&mut slot.cast, ctx.delta).is_some() && slot.weapon.is_none() {
                slot.weapon = Some(SlotStr::looping(SERVANT));
                slot.stood = self.next_stood;
                self.next_stood = self.next_stood.wrapping_add(1);
            }
            tick(&mut slot.shot, ctx.delta);
        }
        EffectStatus::Running
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        for (slot, offset) in self.slots.iter().zip(SERVANT_SLOTS) {
            for s in [&slot.weapon, &slot.cast, &slot.shot].into_iter().flatten() {
                out.push(s.piece(offset, 1.0, true));
            }
        }
    }

    /// A count (0x805a80: `DK_SERVANTWEAPON.wav` at every one, then servants added or taken to match it), or a
    /// skill's spending (one, or as many as stand)
    fn on_message(&mut self, message: EffectMessage) {
        match message {
            EffectMessage::Count(count) => {
                self.sound = true;
                let want = usize::from(count).min(self.slots.len());
                while self.count() < want {
                    self.add();
                }
                while self.count() > want {
                    self.remove(COUNT_DROP);
                }
            }
            EffectMessage::Spent { skill, all } => {
                for _ in 0..if all { self.count() } else { 1 } {
                    self.remove(skill);
                }
            }
        }
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        std::mem::take(&mut self.sound).then_some(SERVANT_SOUND)
    }
}

const ABYSS_CREATE: StrFile = StrFile {
    name: "from_the_abyss/from_the_abyss_shadowball_create/from_the_abyss_shadowball_create",
    secs: 75.0 / 60.0,
};
const ABYSS_BALL: StrFile = StrFile {
    name: "from_the_abyss/from_the_abyss_shadowball/from_the_abyss_shadowball",
    // 60 frames at 60 a second
    secs: 1.0,
};
const ABYSS_DIE: StrFile = StrFile {
    name: "from_the_abyss/from_the_abyss_shadowball_die/from_the_abyss_shadowball_die",
    secs: 18.0 / 60.0,
};

/// Each sphere's size and offset (0x9d1730's table: the size goes to the STR's scale, +0x3fa8)
const ABYSS_SLOTS: [(f32, [f32; 3]); 5] = [
    (0.8, [-4.8, -9.1, 0.0]),
    (0.6, [-3.4, -12.8, 0.0]),
    (0.5, [-0.7, -14.0, 0.0]),
    (0.7, [2.7, -14.0, 0.0]),
    (0.4, [4.3, -10.0, 0.0]),
];

/// An Abyss Chaser's abyss spheres (`CFromTheAbyssEffect`): sphere `i` sits in slot `i`
pub struct FromTheAbyssEffect {
    spheres: [Option<SlotStr>; 5],
    /// The count it last took (+0xd0)
    count: usize,
}

impl FromTheAbyssEffect {
    pub fn new(count: u8) -> Self {
        let mut effect = Self {
            spheres: [None; 5],
            count: 0,
        };
        effect.on_message(EffectMessage::Count(count));
        effect
    }
}

impl Effect for FromTheAbyssEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        for sphere in &mut self.spheres {
            // a sphere that has come goes round as the shadowball; one that has died is gone (0x9d23a0)
            if let Some(ended) = tick(sphere, ctx.delta)
                && ended.file == ABYSS_CREATE
            {
                *sphere = Some(SlotStr::looping(ABYSS_BALL));
            }
        }
        EffectStatus::Running
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        for (sphere, (scale, offset)) in self.spheres.iter().zip(ABYSS_SLOTS) {
            if let Some(s) = sphere {
                out.push(s.piece(offset, scale, false));
            }
        }
    }

    /// 0x9d2b20: a count past the five is not taken; fewer die from the top down, more come from the count up
    fn on_message(&mut self, message: EffectMessage) {
        let EffectMessage::Count(count) = message else {
            return;
        };
        let want = usize::from(count);
        if want > self.spheres.len() {
            return;
        }
        for i in want..self.count {
            if self.spheres[i].is_some() {
                self.spheres[i] = Some(SlotStr::once(ABYSS_DIE));
            }
        }
        for sphere in &mut self.spheres[self.count.min(want)..want] {
            *sphere = Some(SlotStr::once(ABYSS_CREATE));
        }
        self.count = want;
    }
}

/// The coins' faces, by their angle to the camera, and the glow over the coins past the fifth
const COIN_FACES: [&str; 5] = [
    "effect/rl_richs_coin_b/rcoin1.bmp",
    "effect/rl_richs_coin_b/rcoin2.bmp",
    "effect/rl_richs_coin_b/rcoin3.bmp",
    "effect/rl_richs_coin_b/rcoin4.bmp",
    "effect/rl_richs_coin_b/rcoin5.bmp",
];
const COIN_GLOW: &str = "effect/rl_richs_coin_b/light.bmp";
pub const TEXTURES: &[&str] = &[
    COIN_FACES[0],
    COIN_FACES[1],
    COIN_FACES[2],
    COIN_FACES[3],
    COIN_FACES[4],
    COIN_GLOW,
];

/// One of kRO's world steps, at which the coins turn (0.015 rad each)
const COIN_STEP: f32 = 0.016;
const COIN_TURN: f32 = 0.015;
const COIN_RADIUS: f32 = 7.0;
/// How far over the owner they turn (world Y grows downward); kRO's start angle takes the same 10 off (0x956780)
const COIN_HEIGHT: f32 = 10.0;

/// A Rebellion's or Night Watch's coins (`CRichsCoinEffect`): up to five coins round the owner, a glow over each
/// coin past the fifth (0x956780), all turning (0x956a40) and drawn additively over the units, as kRO's capture of
/// the class shows (list +0x160, blend source alpha onto one; coins 2 by 2, glows 3 by 3 at alpha 0x46)
pub struct RichsCoinEffect {
    /// The coins' angles (+0x54, radians)
    coins: Vec<f32>,
    glows: Vec<f32>,
    position: [f32; 3],
}

impl RichsCoinEffect {
    pub fn new(count: u8) -> Self {
        let count = usize::from(count);
        let coins = count.min(COIN_FACES.len());
        let start = |degrees: usize| (degrees as f32).to_radians() - COIN_HEIGHT;
        Self {
            // 360 / n in whole degrees, as kRO divides it
            coins: (0..coins).map(|i| start((360 / coins) * i)).collect(),
            glows: (0..count.saturating_sub(COIN_FACES.len()))
                .map(|j| start(72 * j))
                .collect(),
            position: [0.0; 3],
        }
    }

    fn at(&self, angle: f32) -> [f32; 3] {
        let [x, y, z] = self.position;
        [
            x + angle.sin() * COIN_RADIUS,
            y - COIN_HEIGHT,
            z + angle.cos() * COIN_RADIUS,
        ]
    }
}

/// A coin's face by its angle to the camera's (0x956a40: the angle in degrees less the camera's, ten faces a turn,
/// five pictures twice)
fn coin_face(angle: f32, camera_degrees: f32) -> &'static str {
    let mut degrees = angle.to_degrees() - camera_degrees;
    if degrees > 360.0 {
        degrees = (degrees as i32 % 360) as f32;
    }
    while degrees < 0.0 {
        degrees += 360.0;
    }
    COIN_FACES[(degrees / 36.0) as usize % COIN_FACES.len()]
}

impl Effect for RichsCoinEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        let turn = COIN_TURN * ctx.delta / COIN_STEP;
        for angle in self.coins.iter_mut().chain(self.glows.iter_mut()) {
            *angle = (*angle + turn).rem_euclid(std::f32::consts::TAU);
        }
        EffectStatus::Running
    }

    fn collect_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        let camera = ctx.camera.turn_degrees();
        let coins = self
            .coins
            .iter()
            .map(|&a| (a, coin_face(a, camera), 2.0, 1.0));
        let glows = self
            .glows
            .iter()
            .map(|&a| (a, COIN_GLOW, 3.0, 70.0 / 255.0));
        for (angle, texture, size, alpha) in coins.chain(glows) {
            out.push(EffectPrimitiveDraw::Billboard {
                pos: self.at(angle),
                size: [size, size],
                uv: BILLBOARD_UV,
                rotation: 0.0,
                texture,
                color: [1.0, 1.0, 1.0, alpha],
                blend: BlendKind::Additive,
            });
        }
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.position = pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(delta: f32) -> EffectUpdateCtx {
        EffectUpdateCtx {
            delta,
            camera_target: None,
            caster_yaw: None,
        }
    }

    fn files(effect: &dyn Effect) -> Vec<&'static str> {
        let mut out = Vec::new();
        effect.str_pieces(&mut out);
        out.into_iter().map(|p| p.file).collect()
    }

    #[test]
    fn servants_come_stand_and_shoot_off_as_kros_class_does() {
        // made full and taken to three: two of the coming five shoot off
        let mut s = ServantWeaponEffect::new(3);
        assert_eq!(s.count(), 3);
        assert_eq!(s.take_sfx_request(), Some(SERVANT_SOUND));
        let shoot = servant_shot(COUNT_DROP).name;
        assert_eq!(files(&s).iter().filter(|f| **f == shoot).count(), 2);
        // once played, the coming ones stand and go round
        for _ in 0..40 {
            s.update(&ctx(1.0 / 60.0));
        }
        assert_eq!(files(&s), [SERVANT.name; 3]);
        // a drop takes the one standing longest; of those that stood together, the first slot (0x957960 takes the
        // first that matches)
        s.slots[3].stood = 0;
        s.slots[2].stood = 1;
        s.on_message(EffectMessage::Count(2));
        assert!(s.slots[3].weapon.is_none() && s.slots[3].shot.is_some());
        s.slots[4].stood = 1;
        s.on_message(EffectMessage::Count(1));
        assert!(s.slots[2].weapon.is_none(), "a tie: the lower slot");
        s.on_message(EffectMessage::Count(5));
        assert_eq!(s.count(), 5);
    }

    #[test]
    fn a_skill_spends_servants_with_its_own_farewell() {
        let mut s = ServantWeaponEffect::new(4);
        let sign = servants_spent(SkillEnum::DkServantWSign, Packet::Cast).unwrap();
        s.on_message(sign);
        assert_eq!(s.count(), 3);
        assert!(files(&s).contains(&servant_shot(SkillEnum::DkServantWSign).name));
        let phantom = servants_spent(SkillEnum::DkServantWPhantom, Packet::NoDamage).unwrap();
        s.on_message(phantom);
        assert_eq!(s.count(), 0, "all of them");
        assert!(servants_spent(SkillEnum::DkServantWSign, Packet::NoDamage).is_none());
    }

    #[test]
    fn abyss_spheres_come_go_round_and_die_from_the_top() {
        let mut a = FromTheAbyssEffect::new(2);
        assert_eq!(files(&a), [ABYSS_CREATE.name; 2]);
        let mut pieces = Vec::new();
        a.str_pieces(&mut pieces);
        assert!(
            pieces.iter().all(|p| !p.under_sprites),
            "type 5: over the units"
        );
        for _ in 0..80 {
            a.update(&ctx(1.0 / 60.0));
        }
        assert_eq!(files(&a), [ABYSS_BALL.name; 2]);
        a.on_message(EffectMessage::Count(1));
        assert_eq!(files(&a), [ABYSS_BALL.name, ABYSS_DIE.name]);
        for _ in 0..20 {
            a.update(&ctx(1.0 / 60.0));
        }
        assert_eq!(files(&a), [ABYSS_BALL.name]);
        a.on_message(EffectMessage::Count(6));
        assert_eq!(a.count, 1, "past the five: not taken");
    }

    #[test]
    fn coins_and_glows_sit_where_kros_capture_has_them() {
        // seven: five coins, two glows on the first two coins' places (72 degrees a step either way)
        let c = RichsCoinEffect::new(7);
        assert_eq!((c.coins.len(), c.glows.len()), (5, 2));
        assert_eq!(c.glows, c.coins[..2]);
        let [x, y, z] = c.at(c.coins[0]);
        assert!(((x * x + z * z).sqrt() - COIN_RADIUS).abs() < 1e-4 && y == -COIN_HEIGHT);
        // ten faces a turn: the picture repeats every 180 degrees
        assert_eq!(
            coin_face(0.1, 0.0),
            coin_face(0.1 + std::f32::consts::PI, 0.0)
        );
        assert_eq!(coin_face(0.0, 0.0), COIN_FACES[0]);
        assert_eq!(coin_face(40f32.to_radians(), 0.0), COIN_FACES[1]);
        assert_eq!(renewal(EffectId::Kro1135, false), Renewal::Fresh);
        assert_eq!(renewal(EffectId::Kro1694, true), Renewal::Remove);
        assert_eq!(renewal(EffectId::Kro2109, true), Renewal::Fresh);
        assert_eq!(renewal(EffectId::Kro2109, false), Renewal::Keep);
    }
}
