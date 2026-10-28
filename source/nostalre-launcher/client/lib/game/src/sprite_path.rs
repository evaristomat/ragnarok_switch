pub use models::enums::weapon::WeaponType;

use crate::data_table::name_table::NameTable;
use crate::entity::{EntityCategory, EntityType};
use crate::item_tables::{item_is_shield, item_weapon_view};
use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_table;
use nostalre_resources::Race;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const JT_WARPNPC: u16 = 45;
pub const JT_EFFECTLAUNCHER: u16 = 104;
pub const JT_HIDDEN_NPC: u16 = 111;
pub const JT_HIDDEN_WARP_NPC: u16 = 139;
pub const JT_INVISIBLE: u16 = 32767;

pub const JT_ARCHER_GUARDIAN: u16 = 1285;
pub const JT_KNIGHT_GUARDIAN: u16 = 1286;
pub const JT_SOLDIER_GUARDIAN: u16 = 1287;

pub fn is_guardian(job: u16) -> bool {
    matches!(
        job,
        JT_ARCHER_GUARDIAN | JT_KNIGHT_GUARDIAN | JT_SOLDIER_GUARDIAN
    )
}

pub const SKILL_UNIT_JOB_MIN: u16 = 126;
pub const SKILL_UNIT_JOB_MAX: u16 = 201;

pub const HOMUNCULUS_JOB_MIN: u16 = 6001;
pub const HOMUNCULUS_JOB_MAX: u16 = 6016;
pub const MERCENARY_JOB_MIN: u16 = 6017;
pub const MERCENARY_JOB_MAX: u16 = 6046;

/// Inclusive job-id ranges, first match wins.
const JOB_CATEGORIES: &[(u16, u16, EntityCategory)] = &[
    (0, 44, EntityCategory::Player),
    (JT_WARPNPC, JT_WARPNPC, EntityCategory::WarpPoint),
    (46, 125, EntityCategory::Npc),
    (
        SKILL_UNIT_JOB_MIN,
        SKILL_UNIT_JOB_MAX,
        EntityCategory::Skill,
    ),
    (400, 999, EntityCategory::Npc),
    (1000, 3999, EntityCategory::Monster),
    (4001, 5999, EntityCategory::Player),
    (
        HOMUNCULUS_JOB_MIN,
        HOMUNCULUS_JOB_MAX,
        EntityCategory::Homunculus,
    ),
    (
        MERCENARY_JOB_MIN,
        MERCENARY_JOB_MAX,
        EntityCategory::Mercenary,
    ),
    (10000, 19999, EntityCategory::Npc),
    (JT_INVISIBLE, JT_INVISIBLE, EntityCategory::Invisible),
];

/// Pets are monster jobs, told apart by their head marker — see `Entity::category`.
pub fn entity_category_from_job(job: u16) -> EntityCategory {
    JOB_CATEGORIES
        .iter()
        .find(|(low, high, _)| job >= *low && job <= *high)
        .map_or(EntityCategory::Monster, |(_, _, category)| *category)
}

pub fn entity_type_from_job(job: u16) -> EntityType {
    match entity_category_from_job(job) {
        EntityCategory::Player => EntityType::Player,
        EntityCategory::Npc
        | EntityCategory::WarpPoint
        | EntityCategory::Skill
        | EntityCategory::Cart
        | EntityCategory::Invisible => EntityType::Npc,
        EntityCategory::Monster | EntityCategory::Pet => EntityType::Monster,
        EntityCategory::Homunculus => EntityType::Homunculus,
        EntityCategory::Mercenary => EntityType::Mercenary,
    }
}

pub fn npc_sprite_path(name: &str) -> String {
    nostalre_resources::sprite::npc::of(name)
}

pub fn monster_sprite_path(name: &str) -> String {
    nostalre_resources::sprite::monster::of(name)
}

pub fn homunculus_sprite_path(name: &str) -> String {
    nostalre_resources::sprite::homun::of(name)
}

/// Mercenary bodies live with the human sprites; the name table already carries
/// the sex/type sub-path (backslashes are normalized to `/` by the GRF reader).
pub fn mercenary_sprite_path(name: &str) -> String {
    nostalre_resources::sprite::player::mercenary_body(name)
}

pub fn mercenary_imf_path(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?;
    Some(nostalre_resources::imf::of(base))
}

/// The mercenary weapon sprite sits under `용병`, named `<body>_<weapon>` where
/// the weapon character is the body name's first character (활 bow / 창 spear /
/// 검 sword). The sex sub-path present on the body name is dropped here.
pub fn mercenary_weapon_sprite_path(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?;
    let weapon_char = base.chars().next()?;
    Some(nostalre_resources::sprite::player::mercenary_weapon(
        base,
        weapon_char,
    ))
}

/// Trigger actors that are never drawn. The identity table maps all four to a
/// real sprite name — 104 and 111 to a townsfolk body, 139 to a Poring — so the
/// body has to be suppressed by job id.
pub fn is_undrawn_actor(job: u16) -> bool {
    matches!(
        job,
        JT_WARPNPC | JT_EFFECTLAUNCHER | JT_HIDDEN_NPC | JT_HIDDEN_WARP_NPC
    )
}

/// Undrawn actors that take no click either. The warp point answers with its own
/// cursor and the hidden NPC is a clickable trigger, so neither is listed.
pub fn is_inert_actor(job: u16) -> bool {
    matches!(job, JT_EFFECTLAUNCHER | JT_HIDDEN_WARP_NPC)
}

pub fn entity_sprite_base_path(name_table: &NameTable, job: u16) -> Option<String> {
    let name = name_table.get_name(job)?;
    if is_undrawn_actor(job) {
        return None;
    }
    match entity_type_from_job(job) {
        EntityType::Npc => Some(npc_sprite_path(name)),
        EntityType::Monster => Some(monster_sprite_path(name)),
        EntityType::Homunculus => Some(homunculus_sprite_path(name)),
        EntityType::Mercenary => Some(mercenary_sprite_path(name)),
        EntityType::Player => None,
    }
}

/// Homunculus type index used by the AI (1..16), from the job id.
pub fn homunculus_type_index(job: u16) -> u16 {
    job.saturating_sub(HOMUNCULUS_JOB_MIN) + 1
}

/// Mercenary type index used by the AI (1..30), from the job id.
pub fn mercenary_type_index(job: u16) -> u16 {
    job.saturating_sub(MERCENARY_JOB_MIN) + 1
}

pub const OPTION_FALCON: i32 = 0x10;
pub const OPTION_RIDING: i32 = 0x20;
pub const OPTION_WEDDING: i32 = 0x1000;
pub const OPTION_SANTA: i32 = 0x10000;
pub const OPTION_SUMMER: i32 = 0x40000;
/// The Rune Knight's dragon, one bit per colour: 1 is the green one, 2 to 5 have art only on the Rune Knight
pub const OPTION_DRAGON1: i32 = 0x80000;
pub const OPTION_DRAGON2: i32 = 0x800000;
pub const OPTION_DRAGON3: i32 = 0x1000000;
pub const OPTION_DRAGON4: i32 = 0x2000000;
pub const OPTION_DRAGON5: i32 = 0x4000000;
pub const OPTION_DRAGON: i32 =
    OPTION_DRAGON1 | OPTION_DRAGON2 | OPTION_DRAGON3 | OPTION_DRAGON4 | OPTION_DRAGON5;
pub const OPTION_WUGRIDER: i32 = 0x200000;
pub const OPTION_MADOGEAR: i32 = 0x400000;
pub const OPTION_HANBOK: i32 = 0x8000000;
pub const OPTION_OKTOBERFEST: i32 = 0x10000000;
pub const OPTION_SUMMER2: i32 = 0x20000000;

/// Costume jobs: the body layer uses a whole-body sprite and the weapon and
/// shield layers are not loaded while a costume bit is set.
pub const JT_MARRIED: u16 = 22;
pub const JT_SANTA: u16 = 26;
pub const JT_SUMMER: u16 = 27;
pub const JT_HANBOK: u16 = 28;
pub const JT_OKTOBERFEST: u16 = 29;
pub const JT_SUMMER2: u16 = 30;

/// Bodies kRO picks by a status or the race rather than by a job id, so they have none in its tables
/// (`jobidentity.lub`): ours, past its last job (4361, `JT_2004_JOB_LAST`).
pub const JT_MADOGEAR_SUIT: u16 = 4400;
pub const JT_MEISTER_MADOGEAR_SUIT: u16 = 4401;
pub const JT_DORAM_MARRIED: u16 = 4402;
pub const JT_DORAM_SUMMER: u16 = 4403;
pub const JT_DORAM_HANBOK: u16 = 4404;

/// The costumes a Doram has its own art for (`도람족`); the others it wears as a human
const DORAM_COSTUMES: &[(u16, u16)] = &[
    (JT_MARRIED, JT_DORAM_MARRIED),
    (JT_SUMMER, JT_DORAM_SUMMER),
    (JT_HANBOK, JT_DORAM_HANBOK),
];

/// The costume bits and the job each draws, in the order the client checks them.
const COSTUMES: &[(i32, u16)] = &[
    (OPTION_WEDDING, JT_MARRIED),
    (OPTION_SANTA, JT_SANTA),
    (OPTION_SUMMER, JT_SUMMER),
    (OPTION_HANBOK, JT_HANBOK),
    (OPTION_OKTOBERFEST, JT_OKTOBERFEST),
    (OPTION_SUMMER2, JT_SUMMER2),
];

pub const OPTION_ORCISH: i32 = 0x800;

/// Reverse Orcish (Sage): the head layer is drawn with the orc-face sprite.
pub const ORCFACE_SPRITE_PATH: &str = nostalre_resources::sprite::effect::ORCFACE;

pub fn is_orcish(effect_state: i32) -> bool {
    (effect_state & OPTION_ORCISH) != 0
}

pub fn is_wedding(effect_state: i32) -> bool {
    (effect_state & OPTION_WEDDING) != 0
}

pub fn costume_job(effect_state: i32) -> Option<u16> {
    COSTUMES
        .iter()
        .find(|(option, _)| effect_state & option != 0)
        .map(|(_, job)| *job)
}

pub fn is_costume_job(job: u16) -> bool {
    COSTUMES.iter().any(|(_, costume)| *costume == job)
        || DORAM_COSTUMES.iter().any(|(_, costume)| *costume == job)
}

/// The costume `job` wears: a Doram's own where it has one
fn costume_for(job: u16, costume: u16) -> u16 {
    DORAM_COSTUMES
        .iter()
        .find(|(human, _)| race(job) == Race::Doram && *human == costume)
        .map_or(costume, |(_, doram)| *doram)
}

/// The human costume a Doram's one stands for
fn human_costume(job: u16) -> Option<u16> {
    DORAM_COSTUMES
        .iter()
        .find(|(_, doram)| *doram == job)
        .map(|(human, _)| *human)
}

pub const EFST_RIDING: i16 = 27;
/// The all-job mount (`SC_ALL_RIDING`, the Boarding Halter, `@mount2`): a status, no option bit
pub const EFST_ALL_RIDING: i16 = 613;
/// The Madogear's type (`SC_MADOGEAR`'s val1, `e_mado_type`), with `OPTION_MADOGEAR` for the mount itself
pub const EFST_MADOGEAR_TYPE: i16 = 1149;
const MADO_SUIT: i32 = 2;
pub const EFST_FALCON: i16 = 28;
/// Maya Purple: the wearer sees stealthed actors as black silhouettes.
pub const EFST_CLAIRVOYANCE: i16 = 184;

/// OPTION bits the server delivers only as a bitmask (no status packet), paired with the
/// status-bar icon the client synthesizes for the local player when the bit toggles.
pub const OPTION_STATUS_ICONS: &[(i32, i16)] =
    &[(OPTION_FALCON, EFST_FALCON), (OPTION_RIDING, EFST_RIDING)];

pub fn has_falcon(effect_state: i32) -> bool {
    (effect_state & OPTION_FALCON) != 0
}

pub fn falcon_sprite_path(job: u16) -> &'static str {
    match job {
        4012 => nostalre_resources::sprite::effect::FALCON2,
        _ => nostalre_resources::sprite::effect::FALCON,
    }
}
pub const OPTION_CART_MASK: i32 = 0x08 | 0x80 | 0x100 | 0x200 | 0x400;
pub const OPTION_REMOVABLE_MASK: i32 = OPTION_FALCON | OPTION_RIDING | OPTION_CART_MASK;

pub const OPTION_SIGHT: i32 = 0x01;
pub const OPTION_RUWACH: i32 = 0x2000;
pub const OPTION_HIDE: i32 = 0x02;
pub const OPTION_CLOAK: i32 = 0x04;
pub const OPTION_CHASEWALK: i32 = 0x4000;
pub const OPTION_HIDDEN_MASK: i32 = OPTION_HIDE | OPTION_CLOAK | OPTION_CHASEWALK;

pub const CLOAK_BODY_ALPHA: f32 = 100.0 / 255.0;

/// Relationship of the viewer to the stealthed actor. Self and party members
/// (and, later, the caster's own pet) still see a cloaked body; everyone else
/// sees nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HiddenViewer {
    Own,
    Ally,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HiddenRender {
    Visible,
    /// Draw only the shadow (Hiding for the local player).
    ShadowOnly,
    Alpha(f32),
    /// Body at full opacity with its colour crushed to black, and no shadow.
    Silhouette,
    Skip,
}

pub fn is_hidden(effect_state: i32) -> bool {
    (effect_state & OPTION_HIDDEN_MASK) != 0
}

/// A hidden local player cannot walk unless Tunnel Drive is known; cloak and
/// chase walk never block movement.
pub fn hide_blocks_move(effect_state: i32, knows_tunnel_drive: bool) -> bool {
    (effect_state & OPTION_HIDE) != 0 && !knows_tunnel_drive
}

pub fn hidden_render(effect_state: i32, viewer: HiddenViewer, clairvoyant: bool) -> HiddenRender {
    use HiddenViewer::*;
    let render = if effect_state & OPTION_HIDE != 0 {
        match viewer {
            Own => HiddenRender::ShadowOnly,
            _ => HiddenRender::Skip,
        }
    } else if effect_state & (OPTION_CLOAK | OPTION_CHASEWALK) != 0 {
        match viewer {
            Own | Ally => HiddenRender::Alpha(CLOAK_BODY_ALPHA),
            Other => HiddenRender::Skip,
        }
    } else {
        HiddenRender::Visible
    };
    if clairvoyant && render == HiddenRender::Skip {
        HiddenRender::Silhouette
    } else {
        render
    }
}

/// The mounts: (option bit, job, the job drawn riding). rAthena sends the riding Knight and Crusader lines as their
/// own jobs (`status_set_viewdata`); every later mount comes only as the option bit, and the client swaps the body.
const MOUNTS: &[(i32, u16, u16)] = &[
    (OPTION_RIDING, 7, 13),
    (OPTION_RIDING, 14, 21),
    (OPTION_RIDING, 4008, 4014),
    (OPTION_RIDING, 4015, 4022),
    (OPTION_RIDING, 4030, 4036),
    (OPTION_RIDING, 4037, 4044),
    // the Royal Guard's gryphon
    (OPTION_RIDING, 4066, 4082),
    (OPTION_RIDING, 4073, 4083),
    (OPTION_RIDING, 4102, 4110),
    (OPTION_RIDING, 4258, 4281),
    // the dragon's other colours before the bit for any colour (`JT_RUNE_CHICKEN2` to `5` and their `_H`)
    (OPTION_DRAGON2, 4054, 4088),
    (OPTION_DRAGON3, 4054, 4090),
    (OPTION_DRAGON4, 4054, 4092),
    (OPTION_DRAGON5, 4054, 4094),
    (OPTION_DRAGON2, 4060, 4089),
    (OPTION_DRAGON3, 4060, 4091),
    (OPTION_DRAGON4, 4060, 4093),
    (OPTION_DRAGON5, 4060, 4095),
    (OPTION_DRAGON, 4054, 4080),
    (OPTION_DRAGON, 4060, 4081),
    (OPTION_DRAGON, 4096, 4109),
    (OPTION_DRAGON, 4252, 4280),
    (OPTION_WUGRIDER, 4056, 4084),
    (OPTION_WUGRIDER, 4062, 4085),
    (OPTION_WUGRIDER, 4098, 4111),
    (OPTION_WUGRIDER, 4257, 4278),
    (OPTION_MADOGEAR, 4058, 4086),
    (OPTION_MADOGEAR, 4064, 4087),
    (OPTION_MADOGEAR, 4100, 4112),
    (OPTION_MADOGEAR, 4253, 4279),
];

/// The all-job mount each job rides (job, the job drawn riding), kRO's ids (`jobidentity.lub`), the pairs and bodies
/// as roBrowser draws them (`AllMountTable`) where kRO has the art. A Star Gladiator in union and a Sky Emperor in
/// flight have none and stay on foot. The art holds no weapons or shields: a rider carries none.
#[rustfmt::skip]
const ALL_RIDING: &[(u16, u16)] = &[
    (0, 4124), (1, 4116), (2, 4130), (3, 4122), (4, 4126), (5, 4119), (6, 4141), (7, 4199), (8, 4156), (9, 4134),
    (10, 4138), (11, 4154), (12, 4145), (14, 4203), (15, 4125), (16, 4131), (17, 4142), (18, 4137), (19, 4149),
    (20, 4147), (23, 4128), (24, 4115), (25, 4114), (4001, 4183), (4002, 4184), (4003, 4185), (4004, 4186),
    (4005, 4187), (4006, 4188), (4007, 4189), (4008, 4200), (4009, 4157), (4010, 4136), (4011, 4118), (4012, 4150),
    (4013, 4146), (4015, 4204), (4016, 4139), (4017, 4135), (4018, 4144), (4019, 4121), (4020, 4153), (4021, 4152),
    (4023, 4158), (4024, 4159), (4025, 4160), (4026, 4161), (4027, 4162), (4028, 4163), (4029, 4181), (4030, 4207),
    (4031, 4194), (4032, 4195), (4033, 4196), (4034, 4164), (4035, 4165), (4037, 4210), (4038, 4166), (4039, 4167),
    (4040, 4168), (4041, 4169), (4042, 4170), (4043, 4171), (4045, 4172), (4046, 4155), (4047, 4123), (4049, 4117),
    (4054, 4202), (4055, 4133), (4056, 4198), (4057, 4129), (4058, 4197), (4059, 4140), (4060, 4202), (4061, 4133),
    (4062, 4198), (4063, 4129), (4064, 4197), (4065, 4140), (4066, 4201), (4067, 4132), (4068, 4148), (4069, 4151),
    (4070, 4127), (4071, 4120), (4072, 4143), (4073, 4201), (4074, 4132), (4075, 4148), (4076, 4151), (4077, 4127),
    (4078, 4120), (4079, 4143), (4096, 4209), (4097, 4173), (4098, 4206), (4099, 4174), (4100, 4205), (4101, 4175),
    (4102, 4208), (4103, 4176), (4104, 4177), (4105, 4178), (4106, 4179), (4107, 4180), (4108, 4182), (4190, 4192),
    (4191, 4193), (4211, 4213), (4212, 4214), (4215, 4216), (4218, 4219), (4220, 4221), (4222, 4230), (4223, 4231),
    (4224, 4232), (4225, 4233), (4226, 4234), (4227, 4235), (4228, 4236), (4229, 4237), (4239, 4245), (4240, 4246),
    (4241, 4247), (4242, 4248), (4252, 4265), (4253, 4266), (4254, 4267), (4255, 4268), (4256, 4269), (4257, 4270),
    (4258, 4271), (4259, 4272), (4260, 4273), (4261, 4274), (4262, 4275), (4263, 4276), (4264, 4277), (4302, 4309),
    (4303, 4310), (4304, 4311), (4305, 4312), (4306, 4313), (4307, 4314), (4308, 4315),
];

/// Mounts the server gives as a status instead of an option bit, kept per unit
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusMount {
    /// `EFST_ALL_RIDING` is on
    pub all_riding: bool,
    /// `EFST_MADOGEAR_TYPE` says the suit, not the robot
    pub madogear_suit: bool,
}

impl StatusMount {
    /// Takes in a status change; true when it was one of these.
    pub fn apply(&mut self, efst: i16, active: bool, val1: i32) -> bool {
        match efst {
            EFST_ALL_RIDING => self.all_riding = active,
            EFST_MADOGEAR_TYPE => self.madogear_suit = active && val1 == MADO_SUIT,
            _ => return false,
        }
        true
    }
}

/// The job `job` is drawn as with the mount bits of `effect_state` set.
pub fn mounted_job(job: u16, effect_state: i32) -> Option<u16> {
    MOUNTS
        .iter()
        .find(|(option, base, _)| *base == job && effect_state & option != 0)
        .map(|(_, _, mounted)| *mounted)
}

/// The Madogear robots and the suit each one is worn as (`EFST_MADOGEAR_TYPE`)
const MADOGEAR_SUITS: &[(u16, u16)] = &[(4086, JT_MADOGEAR_SUIT), (4279, JT_MEISTER_MADOGEAR_SUIT)];

fn madogear_suit_job(robot: u16) -> Option<u16> {
    MADOGEAR_SUITS
        .iter()
        .find(|(r, _)| *r == sprite_job(robot))
        .map(|(_, suit)| *suit)
}

fn madogear_robot(suit: u16) -> Option<u16> {
    MADOGEAR_SUITS
        .iter()
        .find(|(_, s)| *s == suit)
        .map(|(robot, _)| *robot)
}

fn all_riding_job(job: u16) -> Option<u16> {
    ALL_RIDING
        .iter()
        .find(|(rider, _)| *rider == job)
        .map(|(_, mount)| *mount)
}

/// The job drawn for `job`: a costume first, then a mount bit, then an all-job mount (only without either, as
/// roBrowser has it).
pub fn visual_job(job: u16, effect_state: i32, mount: StatusMount) -> u16 {
    if let Some(costume) = costume_job(effect_state) {
        return costume_for(job, costume);
    }
    if let Some(mounted) = mounted_job(job, effect_state) {
        return mount
            .madogear_suit
            .then(|| madogear_suit_job(mounted))
            .flatten()
            .unwrap_or(mounted);
    }
    mount
        .all_riding
        .then(|| all_riding_job(job))
        .flatten()
        .unwrap_or(job)
}

/// The jobs drawn beyond the ones rAthena sends: the mounted forms, the all-job mounts, the bodies of a status or a
/// race. Their art must stay in the archive (the GRF audit roots them).
pub fn drawn_jobs() -> impl Iterator<Item = u16> {
    MOUNTS
        .iter()
        .map(|(_, _, mounted)| *mounted)
        .chain(ALL_RIDING.iter().map(|(_, mount)| *mount))
        .chain(MADOGEAR_SUITS.iter().map(|(_, suit)| *suit))
        .chain(DORAM_COSTUMES.iter().map(|(_, costume)| *costume))
}

/// The job a body drawn for a status or a race belongs to (an all-job mount, a suit, a Doram's costume)
fn status_body_rider(job: u16) -> Option<u16> {
    if let Some(robot) = madogear_robot(job) {
        return unmounted_job(robot);
    }
    if human_costume(job).is_some() {
        return Some(4218);
    }
    ALL_RIDING
        .iter()
        .find(|(_, mount)| *mount == job)
        .map(|(rider, _)| *rider)
}

pub fn cart_design_from_option(effect_state: i32) -> Option<u8> {
    match effect_state & OPTION_CART_MASK {
        0x08 => Some(1),
        0x80 => Some(2),
        0x100 => Some(3),
        0x200 => Some(4),
        0x400 => Some(5),
        _ => None,
    }
}

/// The cart `job` pushes for `design`, as kRO picks it (0x752213): a Super Novice always its own (design 0), a baby
/// the baby one (0x883960)
pub fn cart_sprite_path(design: u8, job: u16) -> Option<String> {
    let design = if crate::prayer::is_super_novice(job) {
        0
    } else {
        design
    };
    nostalre_resources::sprite::effect::cart(design, is_baby(job))
}

pub fn unmounted_job(job: u16) -> Option<u16> {
    MOUNTS
        .iter()
        .find(|(_, _, mounted)| *mounted == job)
        .map(|(_, base, _)| *base)
}

pub fn is_baby(job: u16) -> bool {
    matches!(
        job,
        4023..=4045
            | 4096..=4112
            | 4158..=4182
            | 4191
            | 4193..=4196
            | 4205..=4210
            | 4220
            | 4221
            | 4222..=4237
            | 4238
            | 4241
            | 4242
            | 4244
            | 4247
            | 4248
    )
}

/// Baby classes reuse their base job's resources, drawn smaller — first-class
/// babies (and the Baby Super Novice) smaller than second-class ones. `1.0` for
/// every other job.
pub fn baby_body_scale(job: u16) -> f32 {
    if !is_baby(job) {
        return 1.0;
    }
    match sprite_job(job) {
        0..=6 | 23..=25 | 4046 | 4190 | 4218 | 4219 => 0.75,
        _ => 0.82,
    }
}

/// The all-job mounts drawn with another one's art: the baby and transcendent forms, mostly
#[rustfmt::skip]
const MOUNT_ART_ALIASES: &[(u16, u16)] = &[
    (4158, 4124), (4159, 4116), (4160, 4130), (4161, 4122), (4162, 4126), (4163, 4119), (4164, 4154), (4165, 4145),
    (4166, 4125), (4167, 4131), (4168, 4142), (4169, 4137), (4170, 4149), (4171, 4147), (4172, 4128), (4173, 4133),
    (4174, 4129), (4175, 4140), (4176, 4132), (4177, 4148), (4178, 4151), (4179, 4127), (4180, 4120), (4181, 4141),
    (4182, 4143), (4183, 4124), (4184, 4116), (4185, 4130), (4186, 4122), (4187, 4126), (4188, 4119), (4189, 4141),
    (4192, 4128), (4193, 4128), (4194, 4156), (4195, 4134), (4196, 4138), (4205, 4197), (4206, 4198), (4207, 4199),
    (4208, 4201), (4209, 4202), (4210, 4203), (4221, 4219), (4230, 4114), (4231, 4213), (4232, 4214), (4233, 4155),
    (4234, 4123), (4235, 4117), (4236, 4115), (4237, 4216), (4247, 4245), (4248, 4246),
];

/// The job whose art a job is drawn with: a baby draws as its adult job, a transcendent 3rd job as its 3rd job
/// (their sprites are the same). Identity for every other job.
pub fn sprite_job(job: u16) -> u16 {
    match job {
        4089 | 4091 | 4093 | 4095 => job - 1,
        4023..=4044 => job - 4023,
        4045 => 23,
        4060..=4065 => job - 6,
        4073..=4079 => job - 7,
        4081 | 4083 | 4085 | 4087 => job - 1,
        4096..=4101 => job - 42,
        4102..=4108 => job - 36,
        4109 => 4080,
        4110 => 4082,
        4111 => 4084,
        4112 => 4086,
        4191 => 4190,
        4220 => 4218,
        4222 => 25,
        4223 => 4211,
        4224 => 4212,
        4225 => 4046,
        4226 => 4047,
        4227 => 4049,
        4228 => 24,
        4229 => 4215,
        4238 => 4048,
        4241 => 4239,
        4242 => 4240,
        4244 => 4243,
        _ => MOUNT_ART_ALIASES
            .iter()
            .find(|(alias, _)| *alias == job)
            .map_or(job, |(_, art)| *art),
    }
}

/// The name kRO's art carries for a job (`data/sprite/<race>/몸통/<sex>/<name>_<sex>`): Korean for the jobs before
/// the expanded ones, English after.
fn job_name_kr(job_class: u16) -> &'static str {
    match sprite_job(job_class) {
        0 => "초보자",
        1 => "검사",
        2 => "마법사",
        3 => "궁수",
        4 => "성직자",
        5 => "상인",
        6 => "도둑",
        7 => "기사",
        8 => "프리스트",
        9 => "위저드",
        10 => "제철공",
        11 => "헌터",
        12 => "어세신",
        13 => "페코페코_기사",
        14 => "크루세이더",
        15 => "몽크",
        16 => "세이지",
        17 => "로그",
        18 => "연금술사",
        19 => "바드",
        20 => "무희",
        21 => "신페코크루세이더",
        22 => "결혼",
        23 | 4190 => "슈퍼노비스",
        24 => "건너",
        25 => "닌자",
        26 => "산타",
        27 => "여름",
        28 => "한복",
        29 => "옥토버패스트",
        30 => "여름2",
        4001 => "초보자",
        4002 => "검사",
        4003 => "마법사",
        4004 => "궁수",
        4005 => "성직자",
        4006 => "상인",
        4007 => "도둑",
        4008 => "로드나이트",
        4009 => "하이프리",
        4010 => "하이위저드",
        4011 => "화이트스미스",
        4012 => "스나이퍼",
        4013 => "어쌔신크로스",
        4014 => "로드페코",
        4015 => "팔라딘",
        4016 => "챔피온",
        4017 => "프로페서",
        4018 => "스토커",
        4019 => "크리에이터",
        4020 => "클라운",
        4021 => "집시",
        4022 => "페코팔라딘",
        4046 => "태권소년",
        4047 => "권성",
        4048 => "권성융합",
        4049 => "소울링커",
        4054 => "룬나이트",
        4055 => "워록",
        4056 => "레인져",
        4057 => "아크비숍",
        4058 => "미케닉",
        4059 => "길로틴크로스",
        4066 => "가드",
        4067 => "소서러",
        4068 => "민스트럴",
        4069 => "원더러",
        4070 => "슈라",
        4071 => "제네릭",
        4072 => "쉐도우체이서",
        4080 => "룬나이트쁘띠",
        4082 => "그리폰가드",
        4084 => "레인져늑대",
        4086 => "마도기어",
        4088 => "룬나이트쁘띠2",
        4090 => "룬나이트쁘띠3",
        4092 => "룬나이트쁘띠4",
        4094 => "룬나이트쁘띠5",
        4114 => "두꺼비닌자",
        4115 => "페코건너",
        4116 => "페코검사",
        4117 => "두꺼비소울링커",
        4118 => "화이트스미스멧돼지",
        4119 => "상인멧돼지",
        4120 => "제네릭멧돼지",
        4121 => "크리에이터멧돼지",
        4122 => "타조궁수",
        4123 => "권성포링",
        4124 => "노비스포링",
        4125 => "몽크알파카",
        4126 => "복사알파카",
        4127 => "슈라알파카",
        4128 => "슈퍼노비스포링",
        4129 => "아크비숍알파카",
        4130 => "여우마법사",
        4131 => "여우세이지",
        4132 => "여우소서러",
        4133 => "여우워록",
        4134 => "여우위저드",
        4135 => "여우프로페서",
        4136 => "여우하이위저드",
        4137 => "연금술사멧돼지",
        4138 => "제철공멧돼지",
        4139 => "챔피온알파카",
        4140 => "켈베로스길로틴크로스",
        4141 => "켈베로스도둑",
        4142 => "켈베로스로그",
        4143 => "켈베로스쉐도우체이서",
        4144 => "켈베로스스토커",
        4145 => "켈베로스어쎄신",
        4146 => "켈베로스어쎄신크로스",
        4147 => "타조무희",
        4148 => "타조민스트럴",
        4149 => "타조바드",
        4150 => "타조스나이퍼",
        4151 => "타조원더러",
        4152 => "타조짚시",
        4153 => "타조크라운",
        4154 => "타조헌터",
        4155 => "태권소년포링",
        4156 => "프리스트알파카",
        4157 => "하이프리스트알파카",
        4197 => "미케닉멧돼지",
        4198 => "타조레인져",
        4199 => "사자기사",
        4200 => "사자로드나이트",
        4201 => "사자로얄가드",
        4202 => "사자룬나이트",
        4203 => "사자크루세이더",
        4204 => "사자팔라딘",
        4213 => "frog_kagerou",
        4214 => "frog_oboro",
        4216 => "peco_rebellion",
        4245 => "해태성제",
        4246 => "해태소울리퍼",
        4265 => "dragon_knight_riding",
        4266 => "meister_riding",
        4267 => "shadow_cross_riding",
        4268 => "arch_mage_riding",
        4269 => "cardinal_riding",
        4270 => "windhawk_riding",
        4271 => "imperial_guard_riding",
        4272 => "biolo_riding",
        4273 => "abyss_chaser_riding",
        4274 => "elemetal_master_riding",
        4275 => "inquisitor_riding",
        4276 => "troubadour_riding",
        4277 => "trouvere_riding",
        4309 => "sky_emperor_riding",
        4310 => "soul_ascetic_riding",
        4311 => "shinkiro_riding",
        4312 => "shiranui_riding",
        4313 => "night_watch_riding",
        4314 => "hyper_novice_riding",
        4315 => "spirit_handler_riding",
        4211 => "kagerou",
        4212 => "oboro",
        4215 => "rebellion",
        4218 => "summoner",
        4219 => "cart_summoner",
        4239 => "성제",
        4240 => "소울리퍼",
        4243 => "성제융합",
        4252 => "dragon_knight",
        4253 => "meister",
        4254 => "shadow_cross",
        4255 => "arch_mage",
        4256 => "cardinal",
        4257 => "windhawk",
        4258 => "imperial_guard",
        4259 => "biolo",
        4260 => "abyss_chaser",
        4261 => "elemetal_master",
        4262 => "inquisitor",
        4263 => "troubadour",
        4264 => "trouvere",
        4278 => "wolf_windhawk",
        4279 => "meister_madogear1",
        4280 => "dragon_knight_chicken",
        4281 => "imperial_guard_chicken",
        4302 => "sky_emperor",
        4303 => "soul_ascetic",
        4304 => "shinkiro",
        4305 => "shiranui",
        4306 => "night_watch",
        4307 => "hyper_novice",
        4308 => "spirit_handler",
        4316 => "sky_emperor2",
        JT_MADOGEAR_SUIT => "마도아머",
        JT_MEISTER_MADOGEAR_SUIT => "meister_madogear2",
        JT_DORAM_MARRIED => "결혼",
        JT_DORAM_SUMMER => "여름",
        JT_DORAM_HANBOK => "한복",
        _ => "초보자",
    }
}

/// The name of a job's body palettes (`data/palette/몸/<name>_<sex>_<n>.pal`), where it is not the body's.
fn body_palette_name(job_class: u16) -> &'static str {
    match sprite_job(job_class) {
        14 => "크루",
        20 => "댄서",
        21 => "페코페코_크루",
        26 => "santa",
        27 => "summer",
        28 => "hanbok",
        29 => "oktoberfest",
        30 => "summer2",
        4009 => "하이프리스트",
        4013 => "어세신크로스",
        4014 => "페코로나",
        4020 => "크라운",
        4022 => "페코팔라",
        4056 => "레인저",
        4066 => "로얄가드",
        4080 => "룬드래곤",
        4084 => "울프레인저",
        4086 => "미케닉_마도기어",
        JT_MADOGEAR_SUIT => "미케닉_마도아머",
        4215 => "리벨리온",
        4218 => "묘족",
        job => job_name_kr(job),
    }
}

/// The folder of a job's weapons (`data/sprite/<race>/<name>/`), where it is not named as the body.
fn weapon_folder(job_class: u16) -> &'static str {
    match sprite_job(job_class) {
        4066 => "로얄가드",
        4080 => "페코페코_룬나이트",
        4082 => "신페코로얄가드",
        job => job_name_kr(job),
    }
}

/// The folder of a job's shields (`data/sprite/방패/<name>/`), where it is not named as the body.
fn shield_folder(job_class: u16) -> &'static str {
    match sprite_job(job_class) {
        4009 => "하이프리스트",
        4070 => "수라",
        job => job_name_kr(job),
    }
}

/// A Doram (Summoner, Spirit Handler, and their costumes): the stylist offers it lists of its own
pub fn is_doram(job_class: u16) -> bool {
    matches!(race(job_class), Race::Doram)
}

fn race(job_class: u16) -> Race {
    match sprite_job(job_class) {
        4218 | 4219 | 4308 | 4315 => Race::Doram,
        job if human_costume(job).is_some() => Race::Doram,
        _ => Race::Human,
    }
}

fn sex_kr(sex: u8) -> &'static str {
    if sex == 0 { "여" } else { "남" }
}

pub fn body_sprite_path(job_class: u16, sex: u8) -> String {
    let job = job_name_kr(job_class);
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::player::body(race(job_class), job, sex_str)
}

pub fn gm_body_sprite_path(sex: u8) -> String {
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::player::gm_body(sex_str)
}

pub fn gm_weapon_sprite_path(sex: u8) -> String {
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::player::gm_weapon(sex_str)
}

pub fn imf_path(job_class: u16, sex: u8) -> String {
    let job = job_name_kr(job_class);
    let sex_str = sex_kr(sex);
    nostalre_resources::imf::for_job(job, sex_str)
}

/// The job whose IMF a class borrows when it ships none of its own: the one its weapons fall back to, the
/// TaeKwon's for the Star Gladiator, the wedding outfit's for the other costumes.
pub fn imf_fallback_job(job_class: u16) -> Option<u16> {
    match sprite_job(job_class) {
        4047 => Some(4046),
        26..=30 => Some(22),
        job if human_costume(job).is_some() => human_costume(job),
        job => weapon_fallback_job(job),
    }
}

/// The job whose weapon or shield art to try when `job` has none: each job the one it grew from (a 4th job its
/// 3rd, a 3rd or transcendent job its 2nd), a mounted job the mounted one before it (so the weapon keeps the
/// mount's frames), then its unmounted job.
pub fn weapon_fallback_job(job: u16) -> Option<u16> {
    let job = sprite_job(job);
    let fallback = match job {
        4008..=4013 | 4015..=4021 => job - 4001,
        4014 => 13,
        4022 => 21,
        4054 => 7,
        4055 => 9,
        4056 => 11,
        4057 => 8,
        4058 => 10,
        4059 => 12,
        4066 => 14,
        4067 => 16,
        4068 => 19,
        4069 => 20,
        4070 => 15,
        4071 => 18,
        4072 => 17,
        4080 => 13,
        4082 => 21,
        4048 => 4047,
        4190 => 23,
        4211 | 4212 => 25,
        4215 => 24,
        4239 => 4047,
        4240 => 4049,
        4243 => 4048,
        4252 => 4054,
        4253 => 4058,
        4254 => 4059,
        4255 => 4055,
        4256 => 4057,
        4257 => 4056,
        4258 => 4066,
        4259 => 4071,
        4260 => 4072,
        4261 => 4067,
        4262 => 4070,
        4263 => 4068,
        4264 => 4069,
        4278 => 4084,
        4279 => 4086,
        4280 => 4080,
        4281 => 4082,
        4302 => 4239,
        4303 => 4240,
        4304 => 4211,
        4305 => 4212,
        4306 => 4215,
        4307 => 4190,
        4308 => 4218,
        4316 => 4302,
        4088 | 4090 | 4092 | 4094 => 4080,
        // the suit holds what the robot holds
        _ if let Some(robot) = madogear_robot(job) => robot,
        // no weapon art under any of their names, and a rider's own would be drawn standing
        _ if status_body_rider(job).is_some() => return None,
        _ => return unmounted_job(job),
    };
    Some(fallback)
}

/// The job of the lines before the 3rd jobs whose rules (motions, sounds, auras, notices) a job follows: its adult,
/// unmounted form, a 3rd or later job the one it grew from ([`weapon_fallback_job`]).
pub fn classic_job(job: u16) -> u16 {
    let job = sprite_job(job);
    let mut job = status_body_rider(job).unwrap_or(job);
    while job >= FIRST_THIRD_JOB
        && let Some(parent) = weapon_fallback_job(job)
    {
        job = parent;
    }
    unmounted_job(job).unwrap_or(job)
}

/// The first id past the classic lines (rAthena's `JOB_RUNE_KNIGHT`; 4050-4052 are unused monster jobs).
const FIRST_THIRD_JOB: u16 = 4050;

/// The job whose garment art to try when `job` has none: as [`weapon_fallback_job`], but a mounted job never takes
/// the unmounted one's (it would be drawn standing over the mount).
pub fn robe_fallback_job(job: u16) -> Option<u16> {
    weapon_fallback_job(job).filter(|&parent| unmounted_job(sprite_job(job)) != Some(parent))
}

/// The hair style a character wears is an index, not a file number: each sex
/// resolves it through its own table.
const HEAD_NAMES_MALE: [&str; 26] = [
    "2", "2", "1", "7", "5", "4", "3", "6", "8", "9", "10", "12", "11", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24", "25",
];

const HEAD_NAMES_FEMALE: [&str; 26] = [
    "2", "2", "4", "7", "1", "5", "3", "6", "12", "10", "9", "11", "8", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24", "25",
];

/// A Doram's hair is its file number, 1 to 10.
const DORAM_HEADS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];

/// What a hair style the archives lack draws as (`load_head_sprite`); its file is `13` for both sexes
pub const FALLBACK_HAIR_STYLE: u16 = 13;

/// Past the table a hair style is its own file number (`30_여.spr` and up, the stylist's newer styles), as roBrowser
/// reads them (`HairIndexTable[sex][id] || id`)
fn head_name(head_id: u16, job_class: u16, sex: u8) -> std::borrow::Cow<'static, str> {
    if race(job_class) == Race::Doram {
        return DORAM_HEADS[(head_id as usize).clamp(1, DORAM_HEADS.len()) - 1].into();
    }
    let table = if sex == 0 {
        &HEAD_NAMES_FEMALE
    } else {
        &HEAD_NAMES_MALE
    };
    match table.get(head_id as usize) {
        Some(&name) => name.into(),
        None => head_id.to_string().into(),
    }
}

pub fn head_sprite_path(head_id: u16, job_class: u16, sex: u8) -> String {
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::player::head(
        race(job_class),
        &head_name(head_id, job_class, sex),
        sex_str,
    )
}

/// Index 0 means "whatever suits the job", and only the palette resolves it;
/// the sprite keeps the table entry for 0.
pub fn head_palette_path(head_id: u16, job_class: u16, sex: u8, palette_id: u16) -> String {
    let head_id = match head_id {
        // the first jobs' hair, their transcendent and baby forms' too
        0 => match classic_job(job_class) {
            job @ 1..=6 => job + 1,
            job @ 4002..=4007 => job - 4001 + 1,
            _ => 1,
        },
        id => id,
    };
    let sex_str = sex_kr(sex);
    nostalre_resources::palette::head(
        race(job_class),
        &head_name(head_id, job_class, sex),
        sex_str,
        palette_id,
    )
}

pub fn body_palette_path(job_class: u16, sex: u8, palette_id: u16) -> String {
    let sex_str = sex_kr(sex);
    nostalre_resources::palette::body(
        race(job_class),
        body_palette_name(job_class),
        sex_str,
        palette_id,
    )
}

/// kRO's weapontable names (`WeaponNameTable`)
fn weapon_suffix(weapon_type: WeaponType) -> &'static str {
    match weapon_type {
        WeaponType::Dagger => "_단검",
        WeaponType::Sword1H => "_검",
        WeaponType::Sword2H => "_양손검",
        WeaponType::Spear1H => "_창",
        WeaponType::Spear2H => "_양손창",
        WeaponType::Axe1H => "_도끼",
        WeaponType::Axe2H => "_양손도끼",
        WeaponType::Mace | WeaponType::Mace2H => "_클럽",
        WeaponType::Staff | WeaponType::Staff2H => "_롯드",
        WeaponType::Bow => "_활",
        WeaponType::Knuckle => "_너클",
        WeaponType::Musical => "_악기",
        WeaponType::Whip => "_채찍",
        WeaponType::Book => "_책",
        WeaponType::Katar => "_카타르_카타르",
        WeaponType::DoubleDd => "_단검_단검",
        WeaponType::DoubleSs => "_검_검",
        WeaponType::DoubleAa => "_도끼_도끼",
        WeaponType::DoubleDs => "_단검_검",
        WeaponType::DoubleDa => "_단검_도끼",
        WeaponType::DoubleSa => "_검_도끼",
        WeaponType::Revolver => "_권총",
        WeaponType::Rifle => "_라이플",
        WeaponType::Gatling => "_기관총",
        WeaponType::Shotgun | WeaponType::Grenade => "_샷건",
        WeaponType::Huuma => "_수리검",
        _ => "_검",
    }
}

/// What the game data says about hand looks past 16 bits, read once (`load_hand_looks`)
#[derive(Default)]
struct HandLooks {
    /// iteminfo's `ClassNum` per item: the view the item is drawn as. 0 is left out: the English iteminfo gives it to
    /// the items it took from other servers (jRO's Feather Shield), whose look it does not know
    classes: HashMap<u32, u16>,
    /// weapontable's views past the 30 base ones, as the classic look that draws them
    expansions: HashMap<u16, u16>,
    /// The views past the base ones with art of their own: their file suffix
    art_names: HashMap<u16, String>,
}

static HAND_LOOKS: OnceLock<HandLooks> = OnceLock::new();

/// Reads the hand looks from iteminfo (`data_table::load_item_info`) and the GRF's weapontable
pub fn load_hand_looks(grf: &GrfArchive, items: &HashMap<u32, lua_table::ItemInfo>) {
    let lua_table::WeaponTable {
        expansions,
        art_names,
    } = crate::data_table::read_lub(
        grf,
        &[nostalre_resources::lua::WEAPON_TABLE_514_LUB],
        lua_table::parse_weapon_table,
    );
    let looks = HandLooks {
        classes: class_views(
            items,
            crate::data_table::read_view_table(grf, nostalre_resources::table::SHIELD_VIEW),
        ),
        expansions,
        art_names,
    };
    tracing::info!(
        items = looks.classes.len(),
        expansions = looks.expansions.len(),
        art_names = looks.art_names.len(),
        "hand looks loaded"
    );
    let _ = HAND_LOOKS.set(looks);
}

/// iteminfo's classes, then rAthena's view for a shield iteminfo gives none
fn class_views(
    items: &HashMap<u32, lua_table::ItemInfo>,
    shield_views: HashMap<u32, u16>,
) -> HashMap<u32, u16> {
    let mut classes: HashMap<u32, u16> = items
        .iter()
        .filter_map(|(id, item)| Some((*id, item.class_num.filter(|&class| class != 0)?)))
        .collect();
    for (id, view) in shield_views {
        if view != 0 {
            classes.entry(id).or_insert(view);
        }
    }
    classes
}

/// The 16-bit look a hand is drawn with. From 20181121 rAthena sends a weapon without a view id as its item id,
/// 32-bit; like the original game, the client resolves one past 16 bits through the item's `ClassNum`, and a view
/// past the base ones (Lance, Main Gauche...) through weapontable. Looks that fit 16 bits are views or item ids
/// [`weapon_view_id_to_type`] knows.
pub fn weapon_view(look: u32) -> u16 {
    view_of(look, HAND_LOOKS.get().unwrap_or(&HandLooks::default()))
}

/// [`weapon_view`] for the left hand, which holds a shield or a second weapon. A Renewal shield (one rAthena's item
/// db has, sent as its item id like a weapon) is drawn by its `ClassNum`, else as the classic Shield.
pub fn left_hand_view(look: u32) -> u16 {
    left_view_of(look, HAND_LOOKS.get().unwrap_or(&HandLooks::default()))
}

fn left_view_of(look: u32, looks: &HandLooks) -> u16 {
    if look > u16::MAX as u32 && item_is_shield(look) {
        return looks
            .classes
            .get(&look)
            .copied()
            .unwrap_or(CLASSIC_SHIELD as u16);
    }
    // rAthena sends a two-hander once as the left hand alone while it swaps one in (`pc_equipitem` equips the left
    // hand first, and taking the old weapon off the right sends the look in between); the left hand never holds one.
    // A look past the base views is an item, whose view says what it is; a base view number may be a shield's
    let view = view_of(look, looks);
    if look > LAST_BASE_VIEW as u32
        && weapon_view_id_to_type(view).is_some_and(|w| w.is_two_handed())
    {
        return 0;
    }
    view
}

fn view_of(look: u32, looks: &HandLooks) -> u16 {
    if let Ok(look) = u16::try_from(look) {
        return look;
    }
    let view = looks
        .classes
        .get(&look)
        .copied()
        .unwrap_or_else(|| item_weapon_view(look).unwrap_or(0));
    if view <= LAST_BASE_VIEW || looks.art_names.contains_key(&view) {
        return view;
    }
    looks.expansions.get(&view).copied().unwrap_or(0)
}

/// The weapontable view past the base ones a look stands for: the view itself, or an item's `ClassNum`
fn expansion_view(look: u16, looks: &HandLooks) -> Option<u16> {
    let view = if look < WEAPON_LOOK_IS_ITEM_ID {
        look
    } else {
        *looks.classes.get(&(look as u32))?
    };
    (view > LAST_BASE_VIEW).then_some(view)
}

/// Art drawn for a look whose weapontable view has its own (a Doram's foxtails), tried before any other
pub fn weapon_named_sprite_path(job_class: u16, sex: u8, look: u16) -> Option<String> {
    let name = art_name(look, HAND_LOOKS.get()?)?;
    Some(nostalre_resources::sprite::player::weapon(
        race(job_class),
        weapon_folder(job_class),
        sex_kr(sex),
        name,
    ))
}

/// An item whose `ClassNum` is a base weapon view (the Book, 15) is drawn with its type's art, as in kRO, even where
/// the archive has art named after its item id (`세이지_남_1550`, an older book kRO no longer draws)
pub fn weapon_look_is_plain_type(look: u16) -> bool {
    HAND_LOOKS
        .get()
        .is_some_and(|looks| is_plain_type(look, looks))
}

fn is_plain_type(look: u16, looks: &HandLooks) -> bool {
    look >= WEAPON_LOOK_IS_ITEM_ID
        && looks
            .classes
            .get(&(look as u32))
            .is_some_and(|&view| view <= LAST_BASE_VIEW)
}

fn art_name(look: u16, looks: &HandLooks) -> Option<&str> {
    looks
        .art_names
        .get(&expansion_view(look, looks)?)
        .map(String::as_str)
}

/// Weapon looks below this are a weapon-type id, above it an item id. Only the latter can name a sprite of its own.
pub const WEAPON_LOOK_IS_ITEM_ID: u16 = 1100;

/// The last of the base weapon views (`WPCLASS_LAST` - 1 in weapontable is 30, the sword and axe dual wield)
const LAST_BASE_VIEW: u16 = 30;

/// A classic item of each weapon view, whose icon stands in for a weapon newer than the client's iteminfo (which
/// has neither its view nor its icon); the view itself comes from rAthena's item db (`item_tables`)
const WEAPON_STAND_INS: [(u16, u32); 22] = [
    (1, 1201),   // dagger: Knife
    (2, 1101),   // one-handed sword: Sword
    (3, 1116),   // two-handed sword: Katana
    (4, 1401),   // one-handed spear: Javelin
    (5, 1410),   // two-handed spear: Lance
    (6, 1301),   // one-handed axe: Axe
    (7, 1351),   // two-handed axe: Battle Axe
    (8, 1501),   // mace: Club
    (10, 1601),  // staff: Rod
    (11, 1701),  // bow
    (12, 1801),  // knuckle: Waghnak
    (13, 1901),  // instrument: Violin
    (14, 1950),  // whip: Rope
    (15, 1550),  // book
    (16, 1250),  // katar: Jur
    (17, 13100), // revolver: Six Shooter
    (18, 13150), // rifle
    (19, 13157), // gatling: Drifter
    (20, 13154), // shotgun
    (21, 13160), // grenade launcher: Destroyer
    (22, 13300), // huuma
    (23, 2000),  // two-handed staff: Destruction Rod
];

/// The classic Shield (2105)
const CLASSIC_SHIELD: u32 = 2105;

/// A classic weapon or shield of the type of a Renewal one the client's tables do not have, for its icon
pub fn renewal_stand_in_item(item_id: u32) -> Option<u32> {
    if item_id <= u16::MAX as u32 {
        return None;
    }
    if item_is_shield(item_id) {
        return Some(CLASSIC_SHIELD);
    }
    let view = item_weapon_view(item_id)?;
    WEAPON_STAND_INS
        .iter()
        .find(|&&(v, _)| v == view)
        .map(|&(_, classic)| classic)
}

pub fn weapon_view_id_to_type(id: u16) -> Option<WeaponType> {
    match id {
        0 => None,
        1 => Some(WeaponType::Dagger),
        2 => Some(WeaponType::Sword1H),
        3 => Some(WeaponType::Sword2H),
        4 => Some(WeaponType::Spear1H),
        5 => Some(WeaponType::Spear2H),
        6 => Some(WeaponType::Axe1H),
        7 => Some(WeaponType::Axe2H),
        8 => Some(WeaponType::Mace),
        9 => Some(WeaponType::Mace2H),
        10 => Some(WeaponType::Staff),
        11 => Some(WeaponType::Bow),
        12 => Some(WeaponType::Knuckle),
        13 => Some(WeaponType::Musical),
        14 => Some(WeaponType::Whip),
        15 => Some(WeaponType::Book),
        16 => Some(WeaponType::Katar),
        17 => Some(WeaponType::Revolver),
        18 => Some(WeaponType::Rifle),
        19 => Some(WeaponType::Gatling),
        20 => Some(WeaponType::Shotgun),
        21 => Some(WeaponType::Grenade),
        22 => Some(WeaponType::Huuma),
        23 => Some(WeaponType::Staff2H),
        25 => Some(WeaponType::DoubleDd),
        26 => Some(WeaponType::DoubleSs),
        27 => Some(WeaponType::DoubleAa),
        28 => Some(WeaponType::DoubleDs),
        29 => Some(WeaponType::DoubleDa),
        30 => Some(WeaponType::DoubleSa),
        _ => HAND_LOOKS
            .get()
            .and_then(|looks| class_type(id, looks))
            .or_else(|| weapon_type_from_item_id(id)),
    }
}

/// An item's weapon type by its iteminfo `ClassNum`, as the original game reads it: past the classic blocks the
/// guns of every kind share ids (13163 a rifle, 13172 a gatling)
fn class_type(item_id: u16, looks: &HandLooks) -> Option<WeaponType> {
    let class = match expansion_view(item_id, looks) {
        Some(view) => *looks.expansions.get(&view)?,
        None => *looks.classes.get(&(item_id as u32))?,
    };
    (class <= LAST_BASE_VIEW).then(|| weapon_view_id_to_type(class))?
}

/// The weapon type rAthena's item db gives an item the client's iteminfo has no `ClassNum` for
fn weapon_type_from_item_id(id: u16) -> Option<WeaponType> {
    if id < WEAPON_LOOK_IS_ITEM_ID {
        return None;
    }
    item_weapon_view(id as u32).and_then(weapon_view_id_to_type)
}

pub fn dual_wield_type(right: WeaponType, left: WeaponType) -> Option<WeaponType> {
    match (right, left) {
        (WeaponType::Dagger, WeaponType::Dagger) => Some(WeaponType::DoubleDd),
        (WeaponType::Sword1H, WeaponType::Sword1H) => Some(WeaponType::DoubleSs),
        (WeaponType::Axe1H, WeaponType::Axe1H) => Some(WeaponType::DoubleAa),
        (WeaponType::Dagger, WeaponType::Sword1H) | (WeaponType::Sword1H, WeaponType::Dagger) => {
            Some(WeaponType::DoubleDs)
        }
        (WeaponType::Dagger, WeaponType::Axe1H) | (WeaponType::Axe1H, WeaponType::Dagger) => {
            Some(WeaponType::DoubleDa)
        }
        (WeaponType::Sword1H, WeaponType::Axe1H) | (WeaponType::Axe1H, WeaponType::Sword1H) => {
            Some(WeaponType::DoubleSa)
        }
        _ => None,
    }
}

pub fn headgear_sprite_path(suffix: &str, sex: u8) -> String {
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::accessory::of(sex_str, suffix)
}

fn shield_name_kr(view_id: u16) -> Option<&'static str> {
    match view_id {
        1 => Some("가드"),
        2 => Some("버클러"),
        3 => Some("쉴드"),
        4 => Some("미러쉴드"),
        _ => None,
    }
}

fn shield_view_from_item_id(id: u16) -> Option<u16> {
    match id {
        2101 | 2102 | 2112 | 2116..=2120 => Some(1),
        2103 | 2104 | 2114 | 2126 => Some(2),
        2105 | 2106 | 2113 => Some(3),
        2107 | 2108 | 2110 | 2111 | 2115 | 2127 | 2128 => Some(4),
        _ => None,
    }
}

pub fn resolve_shield_view_id(id: u16) -> u16 {
    if (1..=4).contains(&id) {
        return id;
    }
    shield_view_from_item_id(id).unwrap_or(id)
}

pub fn shield_sprite_path(view_id: u16, job_class: u16, sex: u8) -> Option<String> {
    let resolved = resolve_shield_view_id(view_id);
    let shield = shield_name_kr(resolved)?;
    let job = shield_folder(job_class);
    let sex_str = sex_kr(sex);
    Some(nostalre_resources::sprite::shield::of(job, sex_str, shield))
}

/// A garment's sprite for a job (`robe`: its folder, [`RobeTable::name`](crate::data_table::robe_table::RobeTable::name)).
pub fn robe_sprite_path(robe: &str, job_class: u16, sex: u8) -> String {
    nostalre_resources::sprite::robe::of(robe, job_name_kr(job_class), sex_kr(sex))
}

/// A shield the name table does not know is named after its item id (`기사_남_28901_방패`)
pub fn shield_sprite_path_numeric(view_id: u16, job_class: u16, sex: u8) -> String {
    let job = shield_folder(job_class);
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::shield::of(job, sex_str, format!("{view_id}_방패"))
}

pub fn weapon_sprite_path(job_class: u16, sex: u8, weapon_type: WeaponType) -> String {
    let sex_str = sex_kr(sex);
    let suffix = weapon_suffix(weapon_type);
    nostalre_resources::sprite::player::weapon(
        race(job_class),
        weapon_folder(job_class),
        sex_str,
        suffix,
    )
}

/// Weapon art named after the item id rather than the weapon type. The original
/// game reaches for this first and only falls back to [`weapon_sprite_path`]
/// when the archive has no such file.
pub fn weapon_item_sprite_path(job_class: u16, sex: u8, item_id: u16) -> String {
    let sex_str = sex_kr(sex);
    nostalre_resources::sprite::player::weapon_by_item(
        race(job_class),
        weapon_folder(job_class),
        sex_str,
        item_id,
    )
}

pub fn weapon_item_trail_sprite_path(
    job_class: u16,
    sex: u8,
    item_id: u16,
    weapon_type: WeaponType,
) -> Option<String> {
    weapon_has_trail(weapon_type)
        .then(|| format!("{}_검광", weapon_item_sprite_path(job_class, sex, item_id)))
}

pub fn weapon_has_trail(weapon_type: WeaponType) -> bool {
    matches!(
        weapon_type,
        WeaponType::Dagger
            | WeaponType::Sword1H
            | WeaponType::Sword2H
            | WeaponType::Spear1H
            | WeaponType::Spear2H
            | WeaponType::Axe1H
            | WeaponType::Axe2H
            | WeaponType::Katar
            | WeaponType::DoubleDd
            | WeaponType::DoubleSs
            | WeaponType::DoubleAa
            | WeaponType::DoubleDs
            | WeaponType::DoubleDa
            | WeaponType::DoubleSa
            | WeaponType::Revolver
            | WeaponType::Rifle
            | WeaponType::Gatling
            | WeaponType::Shotgun
            | WeaponType::Grenade
    )
}

pub fn weapon_trail_sprite_path(
    job_class: u16,
    sex: u8,
    weapon_type: WeaponType,
) -> Option<String> {
    weapon_has_trail(weapon_type)
        .then(|| format!("{}_검광", weapon_sprite_path(job_class, sex, weapon_type)))
}

/// A Madogear draws no weapon, only its own swing trail, whatever the Mechanic holds and bare-handed too (kRO's
/// 2022 client).
pub fn madogear_trail_sprite_path(job_class: u16, sex: u8) -> Option<String> {
    let name = match sprite_job(job_class) {
        // a robot is the job that has a suit
        job if madogear_suit_job(job).is_some() => "마도기어",
        job if madogear_robot(job).is_some() => "마도아머",
        _ => return None,
    };
    Some(nostalre_resources::sprite::player::madogear_trail(
        name,
        sex_kr(sex),
    ))
}

/// The kick flash a TaeKwon-line actor wears in place of a weapon.
pub fn kick_glow_sprite_path(job_class: u16, sex: u8) -> String {
    nostalre_resources::sprite::player::weapon(
        race(job_class),
        weapon_folder(job_class),
        sex_kr(sex),
        "_발광",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::enums::skill_enums::SkillEnum;

    #[test]
    fn a_hair_style_past_the_table_is_its_own_file() {
        assert!(head_sprite_path(33, 0, 0).ends_with("/33_여"));
        assert!(head_sprite_path(42, 0, 1).ends_with("/42_남"));
        assert!(head_palette_path(33, 0, 0, 2).ends_with("머리33_여_2.pal"));
        assert!(
            head_sprite_path(2, 0, 0).ends_with("/4_여"),
            "the table still names the first ones"
        );
    }

    /// The job drawn with no status-given mount
    fn visual(job: u16, effect_state: i32) -> u16 {
        visual_job(job, effect_state, StatusMount::default())
    }

    #[test]
    fn novice_male_path() {
        assert_eq!(
            body_sprite_path(0, 1),
            "data/sprite/인간족/몸통/남/초보자_남"
        );
    }

    #[test]
    fn novice_female_path() {
        assert_eq!(
            body_sprite_path(0, 0),
            "data/sprite/인간족/몸통/여/초보자_여"
        );
    }

    #[test]
    fn knight_male_path() {
        assert_eq!(body_sprite_path(7, 1), "data/sprite/인간족/몸통/남/기사_남");
    }

    #[test]
    fn lord_knight_has_own_sprite() {
        assert_eq!(
            body_sprite_path(4008, 1),
            "data/sprite/인간족/몸통/남/로드나이트_남"
        );
    }

    #[test]
    fn clown_has_own_sprite() {
        assert_eq!(
            body_sprite_path(4020, 1),
            "data/sprite/인간족/몸통/남/클라운_남"
        );
    }

    #[test]
    fn high_priest_path() {
        assert_eq!(
            body_sprite_path(4009, 1),
            "data/sprite/인간족/몸통/남/하이프리_남"
        );
    }

    #[test]
    fn unknown_class_falls_back_to_novice() {
        assert_eq!(
            body_sprite_path(9999, 1),
            "data/sprite/인간족/몸통/남/초보자_남"
        );
    }

    #[test]
    fn hidden_detects_hide_cloak_and_chasewalk() {
        assert!(is_hidden(OPTION_HIDE));
        assert!(is_hidden(OPTION_CLOAK));
        assert!(is_hidden(OPTION_CHASEWALK));
        assert!(is_hidden(OPTION_RIDING | OPTION_CLOAK));
        assert!(!is_hidden(0));
        assert!(!is_hidden(OPTION_RIDING));
    }

    #[test]
    fn hidden_render_is_per_state_and_viewer_aware() {
        use HiddenRender::*;
        use HiddenViewer::*;
        assert_eq!(hidden_render(0, Own, false), Visible);
        assert_eq!(hidden_render(OPTION_RIDING, Other, false), Visible);

        assert_eq!(
            hidden_render(OPTION_CLOAK, Own, false),
            Alpha(CLOAK_BODY_ALPHA)
        );
        assert_eq!(
            hidden_render(OPTION_CLOAK, Ally, false),
            Alpha(CLOAK_BODY_ALPHA)
        );
        assert_eq!(hidden_render(OPTION_CLOAK, Other, false), Skip);

        assert_eq!(hidden_render(OPTION_HIDE, Own, false), ShadowOnly);
        assert_eq!(hidden_render(OPTION_HIDE, Ally, false), Skip);
        assert_eq!(hidden_render(OPTION_HIDE, Other, false), Skip);

        // Hide beats cloak when both are set.
        assert_eq!(
            hidden_render(OPTION_HIDE | OPTION_CLOAK, Own, false),
            ShadowOnly
        );
        assert_eq!(hidden_render(OPTION_HIDE | OPTION_CLOAK, Ally, false), Skip);

        let chasewalk = OPTION_CHASEWALK | OPTION_CLOAK;
        assert_eq!(
            hidden_render(chasewalk, Own, false),
            Alpha(CLOAK_BODY_ALPHA)
        );
        assert_eq!(
            hidden_render(chasewalk, Ally, false),
            Alpha(CLOAK_BODY_ALPHA)
        );
        assert_eq!(hidden_render(chasewalk, Other, false), Skip);
    }

    #[test]
    fn maya_purple_turns_an_unseen_body_into_a_silhouette() {
        use HiddenRender::*;
        use HiddenViewer::*;
        assert_eq!(hidden_render(OPTION_CLOAK, Other, true), Silhouette);
        assert_eq!(hidden_render(OPTION_HIDE, Other, true), Silhouette);
        assert_eq!(hidden_render(OPTION_HIDE, Ally, true), Silhouette);

        assert_eq!(hidden_render(0, Other, true), Visible);
        assert_eq!(hidden_render(OPTION_HIDE, Own, true), ShadowOnly);
        assert_eq!(
            hidden_render(OPTION_CLOAK, Ally, true),
            Alpha(CLOAK_BODY_ALPHA)
        );
    }

    #[test]
    fn hide_blocks_move_only_without_tunnel_drive() {
        use crate::skill::SkillList;

        let mut list = SkillList::new();
        list.set_skills(vec![crate::skill::SkillData {
            skill: SkillEnum::SmBash,
            level: 5,
            selected_level: 5,
            sp_cost: 10,
            attack_range: 1,
            upgradable: true,
            skill_target_type: crate::skill::SkillTargetType::Target,
        }]);
        let knows = |l: &SkillList| l.get_skill(SkillEnum::RgTunneldrive).is_some();

        assert!(hide_blocks_move(OPTION_HIDE, knows(&list)));
        assert!(!hide_blocks_move(OPTION_CLOAK, knows(&list)));
        assert!(!hide_blocks_move(
            OPTION_CLOAK | OPTION_CHASEWALK,
            knows(&list)
        ));

        list.add_skill(crate::skill::SkillData {
            skill: SkillEnum::RgTunneldrive,
            level: 1,
            selected_level: 1,
            sp_cost: 0,
            attack_range: 0,
            upgradable: false,
            skill_target_type: crate::skill::SkillTargetType::Passive,
        });
        assert!(!hide_blocks_move(OPTION_HIDE, knows(&list)));
    }

    #[test]
    fn imf_path_falls_back_to_the_base_class() {
        assert_eq!(imf_path(6, 1), "data/imf/도둑_남.imf");
        assert_eq!(imf_fallback_job(6), None);
        assert_eq!(imf_path(4030, 1), "data/imf/기사_남.imf");
        assert_eq!(
            imf_fallback_job(4013).map(|j| imf_path(j, 1)),
            Some("data/imf/어세신_남.imf".to_string())
        );
        assert_eq!(
            imf_fallback_job(26).map(|j| imf_path(j, 0)),
            Some("data/imf/결혼_여.imf".to_string())
        );
    }

    #[test]
    fn head_index_maps_per_sex() {
        assert_eq!(
            head_sprite_path(1, 0, 1),
            "data/sprite/인간족/머리통/남/2_남"
        );
        assert_eq!(
            head_sprite_path(2, 0, 1),
            "data/sprite/인간족/머리통/남/1_남"
        );
        assert_eq!(
            head_sprite_path(2, 0, 0),
            "data/sprite/인간족/머리통/여/4_여"
        );
        assert_eq!(
            head_sprite_path(8, 0, 0),
            "data/sprite/인간족/머리통/여/12_여"
        );
        assert_eq!(
            head_sprite_path(20, 0, 0),
            "data/sprite/인간족/머리통/여/20_여"
        );
        // past the table a style is its own file (`official_data.grf` has up to 83); one the archives lack loads
        // the fallback
        assert_eq!(
            head_sprite_path(83, 0, 1),
            "data/sprite/인간족/머리통/남/83_남"
        );
        assert_eq!(
            head_sprite_path(FALLBACK_HAIR_STYLE, 0, 1),
            "data/sprite/인간족/머리통/남/13_남"
        );
    }

    #[test]
    fn knight_dagger_weapon_path() {
        assert_eq!(
            weapon_sprite_path(7, 1, WeaponType::Dagger),
            "data/sprite/인간족/기사/기사_남_단검"
        );
    }

    #[test]
    fn a_hand_look_past_16_bits_is_drawn_by_its_class() {
        // kRO's iteminfo: Boost Sword is a plain sword, Paradise Knight Lance view 52, which weapontable draws as
        // the Lance (1410)
        let looks = HandLooks {
            classes: HashMap::from([(500_001, 2), (630_014, 52)]),
            expansions: HashMap::from([(52, 1410)]),
            ..HandLooks::default()
        };
        assert_eq!(view_of(1201, &looks), 1201);
        assert_eq!(view_of(500_001, &looks), 2);
        assert_eq!(view_of(630_014, &looks), 1410);
        assert_eq!(weapon_view_id_to_type(1410), Some(WeaponType::Spear2H));
        // newer than the client's iteminfo (Awakened Full Force): the item db's type, a two-handed sword
        assert_eq!(view_of(600_025, &looks), 0);
        assert_eq!(view_of(900_000, &looks), 0);
        assert_eq!(view_of(600_025, &HandLooks::default()), 0);
        // the left hand: a Renewal shield by its class, or the classic Shield; a weapon as in the right
        let looks = HandLooks {
            classes: HashMap::from([(460_020, 2)]),
            ..HandLooks::default()
        };
        assert_eq!(left_view_of(460_020, &looks), 2);
        assert_eq!(left_view_of(460_002, &looks), 0);
        assert_eq!(left_view_of(2103, &looks), 2103);
        assert_eq!(left_view_of(510_035, &looks), 0);
    }

    #[test]
    fn an_item_of_a_plain_type_is_drawn_by_its_type() {
        // kRO's iteminfo: the Book is view 15, the Main Gauche 31 (past the base views, its own art)
        let looks = HandLooks {
            classes: HashMap::from([(1550, 15), (1207, 31)]),
            ..HandLooks::default()
        };
        assert!(is_plain_type(1550, &looks));
        assert!(!is_plain_type(1207, &looks));
        // newer than iteminfo: its item-id art may be all there is
        assert!(!is_plain_type(1599, &looks));
        assert!(!is_plain_type(15, &looks), "a view, not an item");
    }

    #[test]
    fn a_two_hander_is_never_the_left_hand() {
        // the swap's in-between look: the Paradise Knight Lance by its class, the Lance, an Awakened Full Force
        // newer than the client's iteminfo (the item db's type), a bow; a Main Gauche in the left hand stays
        let looks = HandLooks {
            classes: HashMap::from([(630_014, 52)]),
            expansions: HashMap::from([(52, 1410)]),
            ..HandLooks::default()
        };
        assert_eq!(left_view_of(630_014, &looks), 0);
        assert_eq!(left_view_of(1410, &looks), 0);
        assert_eq!(left_view_of(600_025, &looks), 0);
        assert_eq!(left_view_of(1701, &looks), 0);
        assert_eq!(left_view_of(1207, &looks), 1207);
        // a base view number may be a shield's: kept
        assert_eq!(left_view_of(3, &looks), 3);
    }

    #[test]
    fn an_item_without_a_class_takes_the_stand_in_look() {
        let item = |class| lua_table::ItemInfo {
            class_num: Some(class),
            ..lua_table::ItemInfo::default()
        };
        // jRO's Feather Shield and Comp_Fatalist, ClassNum 0 in the English iteminfo: rAthena's view draws the shield
        // (the Mirror Shield), the item db's type the dagger (its id block is swords); rAthena never overrides
        // iteminfo's own (460020)
        let items = HashMap::from([
            (460_003, item(0)),
            (460_004, item(0)),
            (500_073, item(0)),
            (460_020, item(2)),
        ]);
        let looks = HandLooks {
            classes: class_views(&items, HashMap::from([(460_003, 4), (460_020, 1)])),
            ..HandLooks::default()
        };
        assert_eq!(left_view_of(460_003, &looks), 4);
        assert_eq!(left_view_of(460_004, &looks), 0);
        assert_eq!(view_of(500_073, &looks), 0);
        assert_eq!(left_view_of(460_020, &looks), 2);
    }

    #[test]
    fn renewal_only_items_have_no_classic_database_fallback() {
        assert_eq!(renewal_stand_in_item(600_025), None);
        assert_eq!(renewal_stand_in_item(500_009), None);
        assert_eq!(renewal_stand_in_item(1201), None);
        assert_eq!(renewal_stand_in_item(900_000), None);
        assert_eq!(renewal_stand_in_item(460_002), None);
        // off their id block's type (the provenance review): a dagger, a mace, a huuma
        assert_eq!(renewal_stand_in_item(500_073), None);
        assert_eq!(renewal_stand_in_item(550_095), None);
        assert_eq!(renewal_stand_in_item(610_072), None);
        // every stand-in is of its view's type in the item db
        for (view, classic) in WEAPON_STAND_INS {
            assert_eq!(item_weapon_view(classic), Some(view), "{classic}");
        }
    }

    #[test]
    fn guns_sharing_an_id_range_keep_their_own_type() {
        assert_eq!(weapon_type_from_item_id(13163), Some(WeaponType::Rifle));
        assert_eq!(weapon_type_from_item_id(13172), Some(WeaponType::Gatling));
        assert_eq!(weapon_type_from_item_id(13167), Some(WeaponType::Shotgun));
        assert_eq!(weapon_type_from_item_id(13174), Some(WeaponType::Grenade));
    }

    #[test]
    fn transcendent_weapons_fall_back_to_the_second_job_mount_kept() {
        let chain = |mut job| {
            let mut jobs = vec![job];
            while let Some(next) = weapon_fallback_job(job) {
                jobs.push(next);
                job = next;
            }
            jobs
        };
        assert_eq!(chain(4014), [4014, 13, 7]);
        assert_eq!(chain(4008), [4008, 7]);
        assert_eq!(chain(4022), [4022, 21, 14]);
        assert_eq!(
            weapon_sprite_path(13, 1, WeaponType::Dagger),
            "data/sprite/인간족/페코페코_기사/페코페코_기사_남_단검"
        );
    }

    #[test]
    fn novice_sword_weapon_path() {
        assert_eq!(
            weapon_sprite_path(0, 1, WeaponType::Sword1H),
            "data/sprite/인간족/초보자/초보자_남_검"
        );
    }

    #[test]
    fn staff_weapon_path() {
        assert_eq!(
            weapon_sprite_path(2, 1, WeaponType::Staff),
            "data/sprite/인간족/마법사/마법사_남_롯드"
        );
    }

    #[test]
    fn female_weapon_path() {
        assert_eq!(
            weapon_sprite_path(7, 0, WeaponType::Spear1H),
            "data/sprite/인간족/기사/기사_여_창"
        );
    }

    #[test]
    fn only_bladed_weapons_have_a_trail() {
        let with_trail = [
            WeaponType::Dagger,
            WeaponType::Sword1H,
            WeaponType::Sword2H,
            WeaponType::Spear1H,
            WeaponType::Spear2H,
            WeaponType::Axe1H,
            WeaponType::Axe2H,
            WeaponType::Katar,
            WeaponType::DoubleDd,
            WeaponType::DoubleSs,
            WeaponType::DoubleAa,
            WeaponType::DoubleDs,
            WeaponType::DoubleDa,
            WeaponType::DoubleSa,
            WeaponType::Revolver,
            WeaponType::Rifle,
            WeaponType::Gatling,
            WeaponType::Shotgun,
            WeaponType::Grenade,
        ];
        let without_trail = [
            WeaponType::Fist,
            WeaponType::Mace,
            WeaponType::Mace2H,
            WeaponType::Staff,
            WeaponType::Staff2H,
            WeaponType::Bow,
            WeaponType::Knuckle,
            WeaponType::Musical,
            WeaponType::Whip,
            WeaponType::Book,
            WeaponType::Huuma,
            WeaponType::Shuriken,
        ];
        for wt in with_trail {
            assert!(weapon_has_trail(wt), "{wt:?} should have a trail");
        }
        for wt in without_trail {
            assert!(!weapon_has_trail(wt), "{wt:?} should have no trail");
            assert!(weapon_trail_sprite_path(7, 1, wt).is_none());
        }
        assert_eq!(
            weapon_trail_sprite_path(7, 1, WeaponType::Sword2H).as_deref(),
            Some("data/sprite/인간족/기사/기사_남_양손검_검광")
        );
    }

    #[test]
    fn taekwon_kick_glow_is_named_after_the_job() {
        assert_eq!(
            kick_glow_sprite_path(4046, 1),
            "data/sprite/인간족/태권소년/태권소년_남_발광"
        );
        assert_eq!(
            kick_glow_sprite_path(4046, 0),
            "data/sprite/인간족/태권소년/태권소년_여_발광"
        );
    }

    #[test]
    fn a_madogear_swings_its_own_trail() {
        let robot = "data/sprite/인간족/마도기어/마도기어_남_검광";
        assert_eq!(madogear_trail_sprite_path(4086, 1).as_deref(), Some(robot));
        assert_eq!(madogear_trail_sprite_path(4112, 1).as_deref(), Some(robot));
        assert_eq!(madogear_trail_sprite_path(4279, 1).as_deref(), Some(robot));
        assert_eq!(
            madogear_trail_sprite_path(JT_MADOGEAR_SUIT, 0).as_deref(),
            Some("data/sprite/인간족/마도기어/마도아머_여_검광")
        );
        assert_eq!(
            madogear_trail_sprite_path(JT_MEISTER_MADOGEAR_SUIT, 1).as_deref(),
            Some("data/sprite/인간족/마도기어/마도아머_남_검광")
        );
        assert_eq!(madogear_trail_sprite_path(4058, 1), None);
    }

    #[test]
    fn weapon_view_id_zero_is_none() {
        assert!(weapon_view_id_to_type(0).is_none());
    }

    #[test]
    fn weapon_view_id_maps_correctly() {
        assert_eq!(weapon_view_id_to_type(1), Some(WeaponType::Dagger));
        assert_eq!(weapon_view_id_to_type(2), Some(WeaponType::Sword1H));
        assert_eq!(weapon_view_id_to_type(11), Some(WeaponType::Bow));
        assert_eq!(weapon_view_id_to_type(16), Some(WeaponType::Katar));
        assert_eq!(weapon_view_id_to_type(25), Some(WeaponType::DoubleDd));
        assert_eq!(weapon_view_id_to_type(30), Some(WeaponType::DoubleSa));
    }

    #[test]
    fn weapon_item_id_fallback() {
        assert_eq!(weapon_view_id_to_type(1101), Some(WeaponType::Sword1H));
        assert_eq!(weapon_view_id_to_type(1201), Some(WeaponType::Dagger));
        assert_eq!(weapon_view_id_to_type(1701), Some(WeaponType::Bow));
        assert_eq!(weapon_view_id_to_type(1250), Some(WeaponType::Katar));
        assert_eq!(weapon_view_id_to_type(1410), Some(WeaponType::Spear2H));
        assert_eq!(weapon_view_id_to_type(1116), Some(WeaponType::Sword2H));
        assert!(weapon_view_id_to_type(999).is_none());
    }

    #[test]
    fn gunslinger_guns_resolve_to_their_art() {
        const GUNSLINGER: u16 = 24;

        // One sprite per kind of gun (kRO's weapontable names).
        assert_eq!(weapon_view_id_to_type(13100), Some(WeaponType::Revolver));
        assert_eq!(
            weapon_sprite_path(GUNSLINGER, 1, WeaponType::Revolver),
            "data/sprite/인간족/건너/건너_남_권총"
        );
        assert_eq!(weapon_view_id_to_type(13150), Some(WeaponType::Rifle));
        assert_eq!(
            weapon_sprite_path(GUNSLINGER, 1, WeaponType::Rifle),
            "data/sprite/인간족/건너/건너_남_라이플"
        );
        assert_eq!(weapon_view_id_to_type(13154), Some(WeaponType::Shotgun));
        assert_eq!(weapon_view_id_to_type(13157), Some(WeaponType::Gatling));
        assert_eq!(
            weapon_sprite_path(GUNSLINGER, 1, WeaponType::Gatling),
            "data/sprite/인간족/건너/건너_남_기관총"
        );
        assert_eq!(weapon_view_id_to_type(13160), Some(WeaponType::Grenade));

        // past the classic blocks the kinds mix: iteminfo's ClassNum says which
        let looks = HandLooks {
            classes: HashMap::from([(13172, 19), (13173, 20)]),
            ..HandLooks::default()
        };
        assert_eq!(class_type(13172, &looks), Some(WeaponType::Gatling));
        assert_eq!(class_type(13173, &looks), Some(WeaponType::Shotgun));
        assert_eq!(class_type(13174, &looks), None);

        // Individual guns carry their own art, keyed by item id.
        assert_eq!(
            weapon_item_sprite_path(GUNSLINGER, 1, 13154),
            "data/sprite/인간족/건너/건너_남_13154"
        );
        assert_eq!(
            weapon_item_trail_sprite_path(GUNSLINGER, 0, 13154, WeaponType::Rifle).as_deref(),
            Some("data/sprite/인간족/건너/건너_여_13154_검광")
        );

        // A weapon look the archive cannot name a file after.
        assert!(weapon_view_id_to_type(13200).is_none()); // bullets are not a look
        assert_eq!(weapon_view_id_to_type(17), Some(WeaponType::Revolver));
    }

    #[test]
    fn a_doram_foxtail_draws_its_own_art_as_a_staff() {
        // Short Foxtail Staff: ClassNum 100, weapontable's WEAPONTYPE_FOXTAIL_GREEN on the staff (10)
        let looks = HandLooks {
            classes: HashMap::from([(1681, 100)]),
            expansions: HashMap::from([(100, 10)]),
            art_names: HashMap::from([(100, "_강아지풀_녹색".to_string())]),
        };
        assert_eq!(art_name(1681, &looks), Some("_강아지풀_녹색"));
        assert_eq!(
            art_name(100, &looks),
            Some("_강아지풀_녹색"),
            "sent as its view"
        );
        assert_eq!(class_type(1681, &looks), Some(WeaponType::Staff));
        assert_eq!(view_of(1681, &looks), 1681, "a 16-bit item id stays itself");
        assert_eq!(art_name(1601, &looks), None, "a plain rod");
    }

    #[test]
    fn entity_type_from_job_boundaries() {
        use crate::entity::EntityType;
        assert_eq!(entity_type_from_job(0), EntityType::Player);
        assert_eq!(entity_type_from_job(44), EntityType::Player);
        assert_eq!(entity_type_from_job(45), EntityType::Npc);
        assert_eq!(entity_type_from_job(46), EntityType::Npc);
        assert_eq!(entity_type_from_job(999), EntityType::Npc);
        assert_eq!(entity_type_from_job(1000), EntityType::Monster);
        assert_eq!(entity_type_from_job(1002), EntityType::Monster);
        assert_eq!(entity_type_from_job(3999), EntityType::Monster);
        assert_eq!(entity_type_from_job(4001), EntityType::Player);
        assert_eq!(entity_type_from_job(5999), EntityType::Player);
        assert_eq!(entity_type_from_job(10000), EntityType::Npc);
    }

    #[test]
    fn entity_category_from_job_boundaries() {
        let cases = [
            (JT_WARPNPC, EntityCategory::WarpPoint),
            (46, EntityCategory::Npc),
            (JT_EFFECTLAUNCHER, EntityCategory::Npc),
            (JT_HIDDEN_NPC, EntityCategory::Npc),
            (125, EntityCategory::Npc),
            (SKILL_UNIT_JOB_MIN, EntityCategory::Skill),
            (JT_HIDDEN_WARP_NPC, EntityCategory::Skill),
            (SKILL_UNIT_JOB_MAX, EntityCategory::Skill),
            (400, EntityCategory::Npc),
            (1000, EntityCategory::Monster),
            (10000, EntityCategory::Npc),
            (19999, EntityCategory::Npc),
            (HOMUNCULUS_JOB_MIN, EntityCategory::Homunculus),
            (MERCENARY_JOB_MAX, EntityCategory::Mercenary),
            (JT_INVISIBLE, EntityCategory::Invisible),
        ];
        for (job, expected) in cases {
            assert_eq!(entity_category_from_job(job), expected, "job {job}");
        }
    }

    #[test]
    fn only_castle_guardians_are_guardians() {
        assert!(is_guardian(JT_ARCHER_GUARDIAN));
        assert!(is_guardian(JT_KNIGHT_GUARDIAN));
        assert!(is_guardian(JT_SOLDIER_GUARDIAN));
        // Emperium and the castle flag.
        assert!(!is_guardian(1288));
        assert!(!is_guardian(722));
    }

    #[test]
    fn body_palette_path_formats_correctly() {
        assert_eq!(body_palette_path(1, 1, 3), "data/palette/몸/검사_남_3.pal");
        assert_eq!(
            body_palette_path(0, 0, 1),
            "data/palette/몸/초보자_여_1.pal"
        );
    }

    #[test]
    fn head_palette_path_formats_correctly() {
        assert_eq!(
            head_palette_path(1, 0, 1, 3),
            "data/palette/머리/머리2_남_3.pal"
        );
        assert_eq!(
            head_palette_path(5, 0, 0, 7),
            "data/palette/머리/머리5_여_7.pal"
        );
    }

    #[test]
    fn head_zero_palette_follows_the_job() {
        assert_eq!(
            head_palette_path(0, 1, 1, 2),
            "data/palette/머리/머리1_남_2.pal"
        );
        assert_eq!(
            head_palette_path(0, 0, 1, 2),
            "data/palette/머리/머리2_남_2.pal"
        );
        assert_eq!(
            head_sprite_path(0, 0, 1),
            "data/sprite/인간족/머리통/남/2_남"
        );
    }

    #[test]
    fn npc_and_monster_sprite_paths() {
        assert_eq!(npc_sprite_path("1_ETC_01"), "data/sprite/npc/1_ETC_01");
        assert_eq!(monster_sprite_path("Poring"), "data/sprite/몬스터/Poring");
    }

    #[test]
    fn shield_view_id_resolution() {
        assert_eq!(resolve_shield_view_id(1), 1);
        assert_eq!(resolve_shield_view_id(2), 2);
        assert_eq!(resolve_shield_view_id(4), 4);
        assert_eq!(resolve_shield_view_id(2101), 1);
        assert_eq!(resolve_shield_view_id(2103), 2);
        assert_eq!(resolve_shield_view_id(2105), 3);
        assert_eq!(resolve_shield_view_id(2107), 4);
    }

    #[test]
    fn shield_sprite_path_with_item_id() {
        let path = shield_sprite_path(2103, 12, 1);
        assert_eq!(path.unwrap(), "data/sprite/방패/어세신/어세신_남_버클러");
    }

    #[test]
    fn visual_job_with_riding() {
        assert_eq!(visual(7, OPTION_RIDING), 13);
        assert_eq!(visual(14, OPTION_RIDING), 21);
        assert_eq!(visual(4008, OPTION_RIDING), 4014);
        assert_eq!(visual(4015, OPTION_RIDING), 4022);
        assert_eq!(visual(0, OPTION_RIDING), 0);
        assert_eq!(visual(12, OPTION_RIDING), 12);
        assert_eq!(visual(7, 0), 7);
        assert_eq!(visual(14, 0), 14);
        assert_eq!(visual(7, 0x01), 7);
    }

    #[test]
    fn baby_class_resolves_to_base_job_sprite_and_rides() {
        assert!(is_baby(4024));
        assert!(!is_baby(1));
        assert_eq!(
            body_sprite_path(4024, 1),
            "data/sprite/인간족/몸통/남/검사_남"
        );
        assert_eq!(sprite_job(4045), 23);
        assert_eq!(visual(4030, OPTION_RIDING), 4036);
        assert_eq!(unmounted_job(4036), Some(4030));
    }

    #[test]
    fn baby_scale_splits_by_class_tier() {
        for first in [4023u16, 4024, 4029, 4045] {
            assert_eq!(baby_body_scale(first), 0.75, "job {first}");
        }
        for second in [4030u16, 4036, 4039, 4044] {
            assert_eq!(baby_body_scale(second), 0.82, "job {second}");
        }
        assert_eq!(baby_body_scale(7), 1.0);
        assert_eq!(baby_body_scale(4008), 1.0);
    }

    #[test]
    fn third_and_fourth_jobs_draw_their_own_bodies() {
        assert_eq!(
            body_sprite_path(4054, 1),
            "data/sprite/인간족/몸통/남/룬나이트_남"
        );
        // transcendent and baby 3rd jobs draw as the 3rd job
        assert_eq!(sprite_job(4060), 4054);
        assert_eq!(sprite_job(4079), 4072);
        assert_eq!(sprite_job(4096), 4054);
        assert_eq!(sprite_job(4102), 4066);
        assert_eq!(sprite_job(4108), 4072);
        assert_eq!(body_sprite_path(4073, 0), body_sprite_path(4066, 0));
        assert_eq!(
            body_sprite_path(4252, 0),
            "data/sprite/인간족/몸통/여/dragon_knight_여"
        );
        assert!(is_baby(4099) && is_baby(4229) && !is_baby(4054));
        assert_eq!(baby_body_scale(4099), 0.82);
        assert_eq!(baby_body_scale(4222), 0.75);
    }

    #[test]
    fn later_mounts_come_as_option_bits() {
        assert_eq!(visual(4054, OPTION_DRAGON1), 4080);
        assert_eq!(visual(4060, OPTION_DRAGON1), 4081);
        assert_eq!(visual(4096, OPTION_DRAGON), 4109);
        assert_eq!(visual(4066, OPTION_RIDING), 4082);
        assert_eq!(visual(4056, OPTION_WUGRIDER), 4084);
        assert_eq!(visual(4058, OPTION_MADOGEAR), 4086);
        assert_eq!(visual(4252, OPTION_DRAGON), 4280);
        assert_eq!(visual(4257, OPTION_WUGRIDER), 4278);
        // a bit that is not the job's own mount changes nothing
        assert_eq!(visual(4054, OPTION_RIDING), 4054);
        assert_eq!(visual(7, OPTION_DRAGON), 7);
        assert_eq!(unmounted_job(4109), Some(4096));
        assert_eq!(
            body_sprite_path(4083, 1),
            "data/sprite/인간족/몸통/남/그리폰가드_남"
        );
    }

    #[test]
    fn the_rune_knight_dragon_comes_in_its_colours() {
        assert_eq!(visual(4054, OPTION_DRAGON3), 4090);
        assert_eq!(visual(4060, OPTION_DRAGON5), 4095);
        assert_eq!(
            body_sprite_path(4095, 0),
            "data/sprite/인간족/몸통/여/룬나이트쁘띠5_여"
        );
        // no colours of their own: the green dragon, the Dragon Knight's chicken
        assert_eq!(visual(4096, OPTION_DRAGON2), 4109);
        assert_eq!(visual(4252, OPTION_DRAGON4), 4280);
        assert_eq!(unmounted_job(4093), Some(4060));
        assert_eq!(weapon_fallback_job(4092), Some(4080));
        assert_eq!(robe_fallback_job(4088), Some(4080));
        assert_eq!(classic_job(4091), 7);
    }

    #[test]
    fn status_mounts_come_with_their_status() {
        let mut mount = StatusMount::default();
        assert!(mount.apply(EFST_ALL_RIDING, true, 1));
        assert!(!mount.apply(EFST_RIDING, true, 0));
        assert_eq!(visual_job(0, 0, mount), 4124);
        assert_eq!(
            body_sprite_path(visual_job(4023, 0, mount), 1),
            "data/sprite/인간족/몸통/남/노비스포링_남"
        );
        assert_eq!(visual_job(4008, 0, mount), 4200);
        assert_eq!(visual_job(4252, 0, mount), 4265);
        assert_eq!(visual_job(4307, 0, mount), 4314);
        // a mount bit or a costume wins, as do jobs without the art
        assert_eq!(visual_job(7, OPTION_RIDING, mount), 13);
        assert_eq!(visual_job(7, OPTION_WEDDING, mount), JT_MARRIED);
        assert_eq!(visual_job(4243, 0, mount), 4243);
        // the Doram riders on their own folder; a baby on the adult's art, drawn small
        assert_eq!(
            body_sprite_path(visual_job(4220, 0, mount), 1),
            "data/sprite/도람족/몸통/남/cart_summoner_남"
        );
        assert_eq!(baby_body_scale(4221), baby_body_scale(4220));
        assert_eq!(
            body_sprite_path(visual_job(4308, 0, mount), 0),
            "data/sprite/도람족/몸통/여/spirit_handler_riding_여"
        );
        assert_eq!(sprite_job(4207), 4199);
        assert!(is_baby(4207) && !is_baby(4199));
        // the rider's rules, no weapon on the mount
        assert_eq!(classic_job(4209), 7);
        assert_eq!(classic_job(4246), 4049);
        assert_eq!(weapon_fallback_job(4199), None);
        assert_eq!(robe_fallback_job(4199), None);

        let mut suit = StatusMount::default();
        suit.apply(EFST_MADOGEAR_TYPE, true, 2);
        assert_eq!(visual_job(4058, OPTION_MADOGEAR, suit), JT_MADOGEAR_SUIT);
        assert_eq!(visual_job(4064, OPTION_MADOGEAR, suit), JT_MADOGEAR_SUIT);
        assert_eq!(
            visual_job(4253, OPTION_MADOGEAR, suit),
            JT_MEISTER_MADOGEAR_SUIT
        );
        assert_eq!(visual_job(4058, 0, suit), 4058);
        assert_eq!(
            body_palette_path(JT_MADOGEAR_SUIT, 1, 2),
            "data/palette/몸/미케닉_마도아머_남_2.pal"
        );
        assert_eq!(classic_job(JT_MADOGEAR_SUIT), 10);
        suit.apply(EFST_MADOGEAR_TYPE, true, 0);
        assert_eq!(visual_job(4058, OPTION_MADOGEAR, suit), 4086);
        suit.apply(EFST_MADOGEAR_TYPE, false, 0);
        assert!(!suit.madogear_suit);
    }

    /// Every job pair is a rider kRO has (`jobidentity.lub`) on a mount with art: each mount names a body.
    #[test]
    fn every_all_job_mount_has_a_body() {
        for &(rider, mount) in ALL_RIDING {
            assert_eq!(entity_category_from_job(rider), EntityCategory::Player);
            assert_ne!(job_name_kr(mount), "초보자", "{rider} rides {mount}");
            assert_eq!(
                status_body_rider(mount).map(classic_job),
                Some(classic_job(rider))
            );
        }
    }

    #[test]
    fn a_doram_wears_its_own_costume_where_it_has_one() {
        assert_eq!(visual(4218, OPTION_WEDDING), JT_DORAM_MARRIED);
        assert_eq!(
            body_sprite_path(visual(4308, OPTION_HANBOK), 1),
            "data/sprite/도람족/몸통/남/한복_남"
        );
        assert_eq!(visual(4220, OPTION_SANTA), JT_SANTA);
        assert_eq!(visual(7, OPTION_SUMMER), JT_SUMMER);
        assert!(is_costume_job(JT_DORAM_SUMMER));
        assert_eq!(imf_fallback_job(JT_DORAM_MARRIED), Some(JT_MARRIED));
    }

    #[test]
    fn palettes_weapons_and_shields_keep_their_own_names() {
        assert_eq!(body_palette_path(14, 1, 2), "data/palette/몸/크루_남_2.pal");
        assert_eq!(
            body_palette_path(4066, 0, 1),
            "data/palette/몸/로얄가드_여_1.pal"
        );
        assert_eq!(
            body_palette_path(4054, 1, 3),
            "data/palette/몸/룬나이트_남_3.pal"
        );
        assert_eq!(
            weapon_sprite_path(4066, 1, WeaponType::Sword1H),
            "data/sprite/인간족/로얄가드/로얄가드_남_검"
        );
        assert_eq!(
            weapon_sprite_path(4109, 1, WeaponType::Spear2H),
            "data/sprite/인간족/페코페코_룬나이트/페코페코_룬나이트_남_양손창"
        );
        assert_eq!(
            shield_sprite_path_numeric(2101, 4070, 1),
            "data/sprite/방패/수라/수라_남_2101_방패"
        );
    }

    #[test]
    fn weapon_art_falls_back_along_the_job_line() {
        let chain = |mut job: u16| {
            let mut jobs = vec![job];
            while let Some(next) = weapon_fallback_job(job) {
                jobs.push(next);
                job = next;
            }
            jobs
        };
        assert_eq!(chain(4252), [4252, 4054, 7]);
        assert_eq!(chain(4280), [4280, 4080, 13, 7]);
        assert_eq!(chain(4062), [4062, 11]);
        assert_eq!(chain(4110), [4110, 21, 14]);
        assert_eq!(chain(4013), [4013, 12]);
        assert_eq!(chain(4316), [4316, 4302, 4239, 4047]);
        assert_eq!(imf_fallback_job(4096), Some(7));
        assert_eq!(imf_fallback_job(JT_SUMMER2), Some(JT_MARRIED));
    }

    #[test]
    fn doram_jobs_draw_from_their_own_folders() {
        assert_eq!(
            body_sprite_path(4220, 0),
            "data/sprite/도람족/몸통/여/summoner_여"
        );
        assert_eq!(
            head_sprite_path(3, 4218, 1),
            "data/sprite/도람족/머리통/남/3_남"
        );
        assert_eq!(
            head_sprite_path(0, 4308, 1),
            "data/sprite/도람족/머리통/남/1_남"
        );
        assert_eq!(
            body_palette_path(4218, 1, 2),
            "data/palette/도람족/body/묘족_남_2.pal"
        );
        assert_eq!(
            head_palette_path(4, 4308, 0, 5),
            "data/palette/도람족/머리/머리4_여_5.pal"
        );
        assert_eq!(
            weapon_sprite_path(4218, 0, WeaponType::Staff),
            "data/sprite/도람족/summoner/summoner_여_롯드"
        );
    }

    #[test]
    fn later_jobs_follow_the_rules_of_their_classic_line() {
        for (job, classic) in [
            (4252, 7),
            (4080, 7),
            (4062, 11),
            (4226, 4047),
            (4014, 4008),
            (4190, 23),
            (4215, 24),
            (4218, 4218),
        ] {
            assert_eq!(classic_job(job), classic, "job {job}");
        }
    }

    #[test]
    fn a_mounted_garment_never_falls_back_to_a_standing_one() {
        assert_eq!(robe_fallback_job(13), None);
        assert_eq!(robe_fallback_job(4084), None);
        assert_eq!(robe_fallback_job(4080), Some(13));
        assert_eq!(robe_fallback_job(4252), Some(4054));
    }

    #[test]
    fn every_job_normalizes_consistently() {
        use models::enums::EnumWithNumberValue;
        use models::enums::class::JobName;
        for job in (0..=u16::MAX).filter(|&j| JobName::try_from_value(j as usize).is_ok()) {
            let art = sprite_job(job);
            assert_eq!(sprite_job(art), art, "job {job}");
            assert_eq!(
                is_baby(job),
                art != job && !(4060..=4087).contains(&job),
                "job {job}"
            );
            let mut chain = vec![job];
            while let Some(next) = weapon_fallback_job(*chain.last().unwrap()) {
                chain.push(next);
                assert!(chain.len() < 6, "job {job}: {chain:?}");
            }
            let classic = classic_job(job);
            assert!(
                classic < FIRST_THIRD_JOB || weapon_fallback_job(classic).is_none(),
                "job {job}"
            );
        }
    }

    #[test]
    fn later_costumes_swap_the_body() {
        assert_eq!(visual(4054, OPTION_HANBOK), JT_HANBOK);
        assert_eq!(visual(7, OPTION_OKTOBERFEST), JT_OKTOBERFEST);
        assert_eq!(visual(7, OPTION_SUMMER2 | OPTION_RIDING), JT_SUMMER2);
        assert!(is_costume_job(JT_SUMMER2));
        assert_eq!(
            body_sprite_path(JT_SUMMER2, 1),
            "data/sprite/인간족/몸통/남/여름2_남"
        );
    }

    #[test]
    fn costume_bits_swap_body_and_win_over_riding() {
        assert_eq!(visual(7, OPTION_WEDDING), JT_MARRIED);
        assert_eq!(visual(7, OPTION_SANTA), JT_SANTA);
        assert_eq!(visual(7, OPTION_SUMMER), JT_SUMMER);
        assert_eq!(visual(7, OPTION_WEDDING | OPTION_RIDING), JT_MARRIED);
        assert_eq!(visual(4008, OPTION_SANTA | OPTION_RIDING), JT_SANTA);
        assert_eq!(visual(7, 0), 7);
        assert_eq!(
            body_sprite_path(JT_MARRIED, 1),
            "data/sprite/인간족/몸통/남/결혼_남"
        );
        assert_eq!(
            body_sprite_path(JT_SANTA, 0),
            "data/sprite/인간족/몸통/여/산타_여"
        );
        assert_eq!(
            body_sprite_path(JT_SUMMER, 1),
            "data/sprite/인간족/몸통/남/여름_남"
        );
    }

    #[test]
    fn gm_sprite_paths_per_sex() {
        assert_eq!(
            gm_body_sprite_path(1),
            "data/sprite/인간족/몸통/남/운영자_남"
        );
        assert_eq!(
            gm_body_sprite_path(0),
            "data/sprite/인간족/몸통/여/운영자_여"
        );
        assert_eq!(
            gm_weapon_sprite_path(1),
            "data/sprite/인간족/운영자/운영자_남_검"
        );
        assert_eq!(
            gm_weapon_sprite_path(0),
            "data/sprite/인간족/운영자/운영자_여_검"
        );
    }

    #[test]
    fn cart_design_from_option_maps_each_bit() {
        assert_eq!(cart_design_from_option(0), None);
        assert_eq!(cart_design_from_option(OPTION_RIDING), None);
        assert_eq!(cart_design_from_option(0x08), Some(1));
        assert_eq!(cart_design_from_option(0x80), Some(2));
        assert_eq!(cart_design_from_option(0x100), Some(3));
        assert_eq!(cart_design_from_option(0x200), Some(4));
        assert_eq!(cart_design_from_option(0x400), Some(5));
        assert_eq!(cart_design_from_option(OPTION_RIDING | 0x100), Some(3));
    }

    #[test]
    fn carts_are_picked_by_design_and_by_who_pushes_them() {
        let cart = |design, job| cart_sprite_path(design, job).unwrap();
        assert_eq!(cart(1, 5), "data/sprite/이팩트/손수레");
        assert_eq!(cart(6, 10), "data/sprite/이팩트/손수레5", "Blacksmith");
        assert_eq!(
            cart(6, 4028),
            "data/sprite/이팩트/손수레5베이비",
            "Baby Merchant"
        );
        assert_eq!(
            cart(2, 4028),
            "data/sprite/이팩트/손수레1",
            "no baby cart before design 6"
        );
        assert_eq!(cart(10, 4028), "data/sprite/이팩트/선물상자카트_미니");
        assert_eq!(cart(12, 4064), "data/sprite/이팩트/포링카트");
        assert_eq!(
            cart(7, 23),
            "data/sprite/이팩트/슈노손수레",
            "a Super Novice pushes its own"
        );
        assert_eq!(cart(13, 4086), "data/sprite/이팩트/마도카트");
        assert_eq!(cart_sprite_path(14, 4064), None, "past kRO's table");
    }

    #[test]
    fn falcon_bit_and_sprite_path() {
        assert!(has_falcon(OPTION_FALCON));
        assert!(has_falcon(OPTION_FALCON | OPTION_RIDING));
        assert!(!has_falcon(0));
        assert!(!has_falcon(OPTION_RIDING));
        assert_eq!(falcon_sprite_path(11), "data/sprite/이팩트/매");
        assert_eq!(falcon_sprite_path(4012), "data/sprite/이팩트/매2");
    }

    #[test]
    fn mounted_job_sprite_paths() {
        assert_eq!(
            body_sprite_path(13, 1),
            "data/sprite/인간족/몸통/남/페코페코_기사_남"
        );
        assert_eq!(
            body_sprite_path(21, 0),
            "data/sprite/인간족/몸통/여/신페코크루세이더_여"
        );
        assert_eq!(
            body_sprite_path(4014, 1),
            "data/sprite/인간족/몸통/남/로드페코_남"
        );
        assert_eq!(
            body_sprite_path(4022, 1),
            "data/sprite/인간족/몸통/남/페코팔라딘_남"
        );
    }
}
