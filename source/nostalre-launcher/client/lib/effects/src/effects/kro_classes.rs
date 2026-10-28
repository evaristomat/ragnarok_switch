//! kRO's registered effect classes with code of their own (0x96aca0's classes past `CEZ2STREffect`), read from the
//! 2022 exe (nostalre-protocol-plan.md 3.6d, "October 18, the night pass"; `scripts/kro_ids.py` reads their ids
//! from here for EffectId). Each plays STRs its code places:
//!
//! - following their owner, lifted, aimed at the target or not (`Follow`: Crimson Arrow's class, Crescive Bolt,
//!   Savage Impact, Petitio, Eternal Slash);
//! - flown to the target (`Flown`: Triangle Shot, Cart Cannon; `StormCannon`; `Boomerang`; `Acidified`;
//!   `Jupitel`'s ball);
//! - dropped about their place on a schedule (`Scatter`: Gale Storm, Diamond Storm; `Jupitel`'s bolts);
//! - set beside their owner (`RandomHit`, `Quake`).
//!
//! The classes step at kRO's 60 updates a second, a step at a time as kRO's 0x992570 runs them, and draw with
//! kRO's `rand()`. An STR plays to the end of its own file and draws nothing after (the STR pipeline): a class
//! whose STRs end by themselves says so (`ends_with_its_strs`), and the holder ends it once they have all played,
//! as only the renderer knows their files' lengths; until then it is held at most `HELD_UPDATES`.
//!
//! A class that needs the target takes the caster and the target from a link (`set_link_endpoints`): the client
//! makes it with both (`effect_queue::is_caster_link_effect`, `needs_target`).

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use models::enums::effect_id::EffectId;

use crate::draw::{BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect_trait::{Effect, EffectRenderCtx, EffectUpdateCtx, StrPiece, StrTurn};
use crate::vec3::{add, length, scale, sub, unit};

const UPDATES_PER_SECOND: f32 = 60.0;
/// The longest a class whose STRs end by themselves is held, should the holder not learn their lengths: past any of
/// their files' (the runs: 16 to 103 updates)
const HELD_UPDATES: u32 = 600;

/// kRO's whole updates, counted from a frame's time
#[derive(Default)]
struct Steps {
    carry: f32,
    done: u32,
}

impl Steps {
    /// The updates `delta` seconds make
    fn advance(&mut self, delta: f32) -> u32 {
        self.carry += delta * UPDATES_PER_SECOND;
        let steps = self.carry as u32;
        self.carry -= steps as f32;
        self.done += steps;
        steps
    }

    fn secs(&self) -> f32 {
        (self.done as f32 + self.carry) / UPDATES_PER_SECOND
    }

    /// Running for as long as a class whose STR ends by itself is held
    fn held(&self) -> EffectStatus {
        if self.done < HELD_UPDATES {
            EffectStatus::Running
        } else {
            EffectStatus::Dead
        }
    }
}

/// `v` turned `degrees` about the world's Y: (x cos a + z sin a, y, −x sin a + z cos a), as 0x941ef0 and 0x941000
/// turn their control points
fn turn_y(v: [f32; 3], degrees: f32) -> [f32; 3] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [v[0] * cos + v[2] * sin, v[1], -v[0] * sin + v[2] * cos]
}

/// The quadratic curve kRO's throws fly: `t²·from + 2t(1−t)·control + (1−t)²·to`
fn curve(from: [f32; 3], control: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    let u = 1.0 - t;
    add(
        add(scale(from, t * t), scale(control, 2.0 * t * u)),
        scale(to, u * u),
    )
}

/// The caster and the target a link gives, both as they were first seen kept apart (kRO reads them once, at
/// creation: 0x992510)
#[derive(Default)]
struct Ends {
    caster: [f32; 3],
    target: [f32; 3],
    caster_at_start: Option<[f32; 3]>,
    target_at_start: Option<[f32; 3]>,
}

impl Ends {
    fn set(&mut self, caster: [f32; 3], target: [f32; 3]) {
        self.caster = caster;
        self.target = target;
        self.caster_at_start.get_or_insert(caster);
        self.target_at_start.get_or_insert(target);
    }

    fn first_target(&self) -> [f32; 3] {
        self.target_at_start.unwrap_or(self.target)
    }
}

/// One or more STRs following their owner, lifted, and turned toward the target as it stood at the start when the
/// class aims (`CCrimsonArrowEffect` 0x9cd210, `CCresciveBolt` 0x9ccd40, `CSavageImpactEffect`) or not
/// (`CPetitioEffect`, `CEternalSlashAttackEffect`); the owner is the unit it is made on, or a link's caster
struct Follow {
    files: &'static [(&'static str, bool)],
    lift: f32,
    /// The aim's turn past the target, for a class that aims
    aim: Option<f32>,
    sound: Option<&'static str>,
    owner: [f32; 3],
    target: Option<[f32; 3]>,
    steps: Steps,
}

impl Effect for Follow {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        self.steps.held()
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let at = add(self.owner, [0.0, self.lift, 0.0]);
        let turn = match (self.aim, self.target) {
            (Some(plus), Some(point)) => StrTurn::Toward { point, plus },
            _ => StrTurn::None,
        };
        for &(file, under_sprites) in self.files {
            out.push(StrPiece {
                at: Some(at),
                turn,
                under_sprites,
                ..StrPiece::new(file, self.steps.secs(), false)
            });
        }
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.owner = pos;
    }

    fn set_link_endpoints(&mut self, caster: [f32; 3], target: [f32; 3]) {
        self.owner = caster;
        self.target.get_or_insert(target);
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        self.sound.take()
    }

    fn ends_with_its_strs(&self) -> bool {
        true
    }
}

/// The arrow or ball flown in a straight line from the caster to the target (`CTriangleShotArrow` 0x95f3c0,
/// `CCartCannon` 0x942d80): from 4 past the caster, lifted 10, a step a update, `per_step` of the way; a way
/// shorter than 40 takes longer steps, as many fewer
struct Flown {
    file: &'static str,
    per_step: f32,
    /// The updates it lives on a way of 40 or more
    updates: f32,
    plus: f32,
    delay_updates: u32,
    ends: Ends,
    steps: Steps,
}

const FLOWN_LIFT: [f32; 3] = [0.0, -10.0, 0.0];
const FLOWN_SHORT: f32 = 40.0;

impl Flown {
    /// (where it starts, its step, the last update it lives)
    fn path(&self) -> ([f32; 3], [f32; 3], f32) {
        let from = add(
            self.ends.caster_at_start.unwrap_or(self.ends.caster),
            FLOWN_LIFT,
        );
        let to = add(self.ends.first_target(), FLOWN_LIFT);
        let way = sub(to, from);
        let len = length(way);
        let k = if len > 0.0 && len < FLOWN_SHORT {
            FLOWN_SHORT / len
        } else {
            1.0
        };
        let base = add(from, scale(unit(way), 4.0));
        (base, scale(way, self.per_step * k), self.updates / k)
    }
}

impl Effect for Flown {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        let (_, _, last) = self.path();
        if (self.steps.done.saturating_sub(self.delay_updates) as f32) <= last {
            EffectStatus::Running
        } else {
            EffectStatus::Dead
        }
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let Some(n) = self.steps.done.checked_sub(self.delay_updates) else {
            return;
        };
        let (base, step, _) = self.path();
        let at = add(base, scale(step, n as f32));
        out.push(StrPiece {
            at: Some(at),
            turn: StrTurn::Toward {
                point: add(self.ends.first_target(), FLOWN_LIFT),
                plus: self.plus,
            },
            ..StrPiece::new(self.file, n as f32 / UPDATES_PER_SECOND, true)
        });
    }

    fn set_link_endpoints(&mut self, caster: [f32; 3], target: [f32; 3]) {
        self.ends.set(caster, target);
    }
}

/// Storm Cannon (`CStormCannonEffect` 0x9d9e10): a cannon ball and its shadow below the units rolling 1.7 a update
/// from the caster to the target, fading by 10 of 255 a update once past it
struct StormCannon {
    ends: Ends,
    steps: Steps,
    sound: Option<&'static str>,
}

const STORM_SPEED: f32 = 1.7;
const STORM_FADE: f32 = 10.0;
const STORM_OFFSET: [f32; 3] = [-1.5, -17.0, 0.0];

impl StormCannon {
    /// (where it is, its alpha out of 1), `None` once faded out
    fn at(&self) -> Option<([f32; 3], f32)> {
        let from = self.ends.caster_at_start?;
        let way = sub(self.ends.first_target(), from);
        let travelled = self.steps.done as f32 * STORM_SPEED;
        let len = length(way);
        let past = ((travelled - len) / STORM_SPEED).max(0.0).floor();
        let alpha = (255.0 - past * STORM_FADE) / 255.0;
        (alpha > 0.0).then(|| {
            (
                add(
                    add(from, scale(unit(way), travelled.min(len))),
                    STORM_OFFSET,
                ),
                alpha,
            )
        })
    }
}

impl Effect for StormCannon {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        if self.ends.caster_at_start.is_none() || self.at().is_some() {
            EffectStatus::Running
        } else {
            EffectStatus::Dead
        }
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let Some((at, alpha)) = self.at() else {
            return;
        };
        let time = self.steps.secs();
        out.push(StrPiece {
            at: Some(at),
            alpha,
            under_sprites: true,
            ..StrPiece::new(
                "stormcannon/stormcannon_bottom/stormcannon_bottom",
                time,
                true,
            )
        });
        out.push(StrPiece {
            at: Some(at),
            alpha,
            ..StrPiece::new("stormcannon/stormcannon/stormcannon", time, true)
        });
    }

    fn set_link_endpoints(&mut self, caster: [f32; 3], target: [f32; 3]) {
        self.ends.set(caster, target);
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        self.sound.take()
    }
}

/// The axe (or shield) thrown out to the target on a curve and back (`CAxeBoomerang` 0x941ef0): out with `t` from
/// 1 down by 0.05 a update, its curve bent 100° by 20; back up by 0.02, bent 260° by 40, both about the target
/// where it stands now
struct Boomerang {
    file: &'static str,
    ends: Ends,
    t: f32,
    back: bool,
    steps: Steps,
}

impl Boomerang {
    fn at(&self) -> [f32; 3] {
        let (angle, side) = if self.back {
            (260.0, 40.0)
        } else {
            (100.0, 20.0)
        };
        let (from, to) = (self.ends.caster, self.ends.target);
        let control = add(to, turn_y(scale(unit(sub(to, from)), side), angle));
        curve(from, control, to, self.t)
    }
}

impl Effect for Boomerang {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        for _ in 0..self.steps.advance(ctx.delta) {
            if self.back {
                self.t += 0.02;
                if self.t >= 1.0 {
                    return EffectStatus::Dead;
                }
            } else {
                self.t -= 0.05;
                if self.t <= 0.0 {
                    self.back = true;
                }
            }
        }
        EffectStatus::Running
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        out.push(StrPiece {
            at: Some(self.at()),
            ..StrPiece::new(self.file, self.steps.secs(), true)
        });
    }

    fn set_link_endpoints(&mut self, caster: [f32; 3], target: [f32; 3]) {
        self.ends.set(caster, target);
    }
}

/// The three places `CRandomHitEffect` takes in turn, for both its ids (0x9d8700: one counter for all, 0x115c6e0):
/// its offset, its turn on screen, its start delay
static RANDOM_HIT_TURN: AtomicU8 = AtomicU8::new(0);
const RANDOM_HITS: [([f32; 3], f32, f32); 3] = [
    ([0.0, -7.0, 0.0], 0.0, 0.0),
    ([4.0, -6.0, 0.0], 320.0, 0.2),
    ([-4.0, -5.0, 0.0], 40.0, 0.4),
];

/// A hit of Second Faith or Second Judgement on its target (`CRandomHitEffect`): one STR at one of three places,
/// run at twice its speed (0x9d88f0 runs the player twice a update)
struct RandomHit {
    file: &'static str,
    place: usize,
    position: [f32; 3],
    steps: Steps,
}

impl Effect for RandomHit {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        self.steps.held()
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let (offset, turn, delay) = RANDOM_HITS[self.place];
        let time = self.steps.secs() - delay;
        if time < 0.0 {
            return;
        }
        out.push(StrPiece {
            at: Some(add(self.position, offset)),
            turn: StrTurn::Fixed(turn),
            ..StrPiece::new(self.file, time * 2.0, false)
        });
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.position = pos;
    }

    fn ends_with_its_strs(&self) -> bool {
        true
    }
}

/// A small pick among a class's files, made from the place it plays (a hash, not kRO's `rand()`), so the same cell
/// picks the same
fn pick(position: [f32; 3], among: usize) -> usize {
    let bits = position[0].to_bits() ^ position[2].to_bits().rotate_left(16);
    (bits.wrapping_mul(0x9e37_79b9) >> 16) as usize % among
}

/// Violent Quake's and All Bloom's attacks at a cell (`CViolentQuakeEffect` 0x9db330): one of the files, 8 up the
/// screen; All Bloom's with its floor under the units
struct Quake {
    files: &'static [(&'static str, Option<&'static str>)],
    sound: Option<&'static str>,
    choice: Option<usize>,
    steps: Steps,
}

const VIOLENT_QUAKE: &[(&str, Option<&str>)] = &[
    ("violentquake/violentquake/violentquake_01", None),
    ("violentquake/violentquake/violentquake_02", None),
    ("violentquake/violentquake/violentquake_03", None),
];
const ALL_BLOOM: &[(&str, Option<&str>)] = &[
    (
        "allbloom/allbloom_bomb/allbloom_bomb01",
        Some("allbloom/allbloom_bomb/allbloom_bomb01_bottom"),
    ),
    (
        "allbloom/allbloom_bomb/allbloom_bomb02",
        Some("allbloom/allbloom_bomb/allbloom_bomb02_bottom"),
    ),
];

impl Effect for Quake {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        self.steps.held()
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let (file, floor) = self.files[self.choice.unwrap_or(0)];
        let time = self.steps.secs();
        out.push(StrPiece {
            view_offset: [0.0, -8.0, 0.0],
            ..StrPiece::new(file, time, false)
        });
        if let Some(floor) = floor {
            out.push(StrPiece {
                under_sprites: true,
                ..StrPiece::new(floor, time, false)
            });
        }
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.choice
            .get_or_insert_with(|| pick(pos, self.files.len()));
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        self.sound.take()
    }

    fn ends_with_its_strs(&self) -> bool {
        true
    }
}

/// The acid bottle thrown at the target and its burst (`CAcidifiedThrow` 0x940930): the bottle on a curve from 10
/// over the caster, `u` from 1 down by 0.07 a update, bent 120° when the target stands left of the caster on screen
/// (else 240°); at update 14 it lands, its sounds play, and the burst plays once at the target
struct Acidified {
    element: &'static AcidElement,
    ends: Ends,
    /// Whether the target stands left of the caster on screen, read at each draw (kRO reads it in camera space);
    /// none before the first, when the bottle is not drawn yet
    target_left: Cell<Option<bool>>,
    steps: Steps,
    sounds: [Option<&'static str>; 2],
}

struct AcidElement {
    bottle: &'static str,
    burst: [&'static str; 2],
    floor: &'static str,
    sound: &'static str,
}

const ACID_LANDS: u32 = 14;
const ACID_STEP: f32 = 0.07;
const ACID_LIFT: [f32; 3] = [0.0, -10.0, 0.0];
const ACID_BURST_OFFSET: [f32; 3] = [-1.0, -14.0, 0.0];
const ACID_SOUND: &str = "effect\\BO_ACIDIFIED_ZONE.wav";

macro_rules! acid {
    ($el:literal, $sound:literal) => {
        AcidElement {
            bottle: concat!(
                "acidified_zone_",
                $el,
                "/acidified_zone_",
                $el,
                "_c/acidified_zone_",
                $el,
                "_c"
            ),
            burst: [
                concat!(
                    "acidified_zone_",
                    $el,
                    "/acidified_zone_",
                    $el,
                    "_b/acidified_zone_",
                    $el,
                    "_b"
                ),
                concat!(
                    "acidified_zone_",
                    $el,
                    "/acidified_zone_",
                    $el,
                    "_water/acidified_zone_",
                    $el,
                    "_water"
                ),
            ],
            floor: concat!(
                "acidified_zone_",
                $el,
                "/acidified_zone_",
                $el,
                "_b_bottom/acidified_zone_",
                $el,
                "_b_bottom"
            ),
            sound: concat!("effect\\BO_ACIDIFIED_ZONE_", $sound, ".wav"),
        }
    };
}

static ACID_FIRE: AcidElement = acid!("fire", "FIRE");
static ACID_WATER: AcidElement = acid!("water", "WATER");
static ACID_WIND: AcidElement = acid!("wind", "WIND");
static ACID_GROUND: AcidElement = acid!("ground", "GROUND");

impl Effect for Acidified {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        let before = self.steps.done;
        self.steps.advance(ctx.delta);
        if before < ACID_LANDS && self.steps.done >= ACID_LANDS {
            self.sounds = [Some(ACID_SOUND), Some(self.element.sound)];
        }
        self.steps.held()
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        // kRO's camera-space x of the two: along the camera's right, its forward crossed with the world's up (-Y),
        // as the renderer's view matrix has it
        let [ex, _, ez] = ctx.camera.eye;
        let [tx, _, tz] = ctx.camera.target;
        let right = [tz - ez, 0.0, -(tx - ex)];
        let way = sub(self.ends.target, self.ends.caster);
        self.target_left
            .set(Some(way[0] * right[0] + way[2] * right[2] < 0.0));
    }

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let n = self.steps.done;
        let target = self.ends.target;
        if n < ACID_LANDS {
            // the renderer reads a class's STRs before its draws: the side is known from the second frame on
            let Some(left) = self.target_left.get() else {
                return;
            };
            let from = add(self.ends.caster, ACID_LIFT);
            let angle = if left { 120.0 } else { 240.0 };
            let control = add(target, turn_y(sub(target, from), angle));
            let u = 1.0 - ACID_STEP * n as f32;
            out.push(StrPiece {
                at: Some(curve(from, control, target, u)),
                ..StrPiece::new(self.element.bottle, self.steps.secs(), true)
            });
            return;
        }
        let time = (n - ACID_LANDS) as f32 / UPDATES_PER_SECOND;
        let at = add(target, ACID_BURST_OFFSET);
        for file in self.element.burst {
            out.push(StrPiece {
                at: Some(at),
                ..StrPiece::new(file, time, false)
            });
        }
        out.push(StrPiece {
            at: Some(at),
            under_sprites: true,
            ..StrPiece::new(self.element.floor, time, false)
        });
    }

    fn set_link_endpoints(&mut self, caster: [f32; 3], target: [f32; 3]) {
        self.ends.set(caster, target);
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        self.sounds.iter_mut().find_map(Option::take)
    }

    fn ends_with_its_strs(&self) -> bool {
        true
    }
}

/// kRO's `rand()`, the C runtime's (`seed·214013 + 2531011`, its 15 bits from 16), one sequence for every class as
/// the exe has one
static RAND_SEED: AtomicU32 = AtomicU32::new(1);

fn rand() -> i32 {
    let next = |seed: u32| seed.wrapping_mul(214_013).wrapping_add(2_531_011);
    let seed = RAND_SEED
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |seed| {
            Some(next(seed))
        })
        .unwrap_or(1);
    ((next(seed) >> 16) & 0x7fff) as i32
}

/// A random spread of `±half` about 0 in steps of `step`, as the classes draw one (`rand()%(2·half+1) − half`)
fn spread(half: i32, step: f32) -> f32 {
    (rand() % (2 * half + 1) - half) as f32 * step
}

/// STRs a class drops one by one where it stands, each playing once where it fell (`CGaleStorm` 0x9d45c0,
/// `CSevereRainStorm`'s Diamond Storm, 0x958860)
struct Scatter {
    kind: ScatterKind,
    /// How wide it scatters (Gale Storm's `n` by the skill's level)
    n: i32,
    owner: [f32; 3],
    dropped: Vec<(&'static str, [f32; 3], u32)>,
    sound: Option<&'static str>,
    steps: Steps,
}

#[derive(Clone, Copy)]
enum ScatterKind {
    /// Gale Storm: a storm every 100/n updates and an arrow every 30/n, both ±2.5n about the target, for 60
    /// updates; an arrow sounds one time in seven
    GaleStorm,
    /// Diamond Storm: one every 5 updates, ±22.5 about the cell, for 60 updates
    DiamondStorm,
}

const SCATTER_UPDATES: u32 = 60;
const DIAMOND_EVERY: u32 = 5;

impl ScatterKind {
    fn storm_every(n: i32) -> u32 {
        (100 / n).max(1) as u32
    }

    fn arrow_every(n: i32) -> u32 {
        (30 / n).max(1) as u32
    }

    /// How many STRs it drops in all, to hold them without growing
    fn drops(self, n: i32) -> usize {
        let over = |every: u32| (SCATTER_UPDATES / every + 1) as usize;
        match self {
            Self::GaleStorm => over(Self::storm_every(n)) + over(Self::arrow_every(n)),
            Self::DiamondStorm => over(DIAMOND_EVERY),
        }
    }
}
const GALE_STORM: &str = "galestorm/galestorm/galestorm";
const GALE_ARROW: &str = "galestorm/galestorm_arrow/galestorm_arrow";
const DIAMOND_STORM: &str = "diamond_storm/diamond_storm/diamond_storm";

impl Scatter {
    fn drop_at(&mut self, file: &'static str, offset: [f32; 2]) {
        let at = add(self.owner, [offset[0], 0.0, offset[1]]);
        self.dropped.push((file, at, self.steps.done));
    }

    /// What update `age` drops
    fn step(&mut self, age: u32) {
        match self.kind {
            ScatterKind::GaleStorm => {
                let n = self.n;
                let half = 5 * n / 2;
                let around = || [spread(half, 1.0), spread(half, 1.0)];
                if age.is_multiple_of(ScatterKind::storm_every(n)) {
                    let at = around();
                    self.drop_at(GALE_STORM, at);
                }
                if age.is_multiple_of(ScatterKind::arrow_every(n)) {
                    let at = around();
                    self.drop_at(GALE_ARROW, at);
                    if rand() % 7 == 0 {
                        self.sound = Some("effect\\WH_GALESTORM.wav");
                    }
                }
            }
            ScatterKind::DiamondStorm => {
                if age.is_multiple_of(DIAMOND_EVERY) {
                    let at = [spread(225, 0.1), spread(225, 0.1)];
                    self.drop_at(DIAMOND_STORM, at);
                }
            }
        }
    }
}

impl Effect for Scatter {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        let first = self.steps.done;
        let steps = self.steps.advance(ctx.delta);
        for age in first..first + steps {
            if age <= SCATTER_UPDATES {
                self.step(age);
            }
        }
        if self.steps.done <= SCATTER_UPDATES + HELD_UPDATES {
            EffectStatus::Running
        } else {
            EffectStatus::Dead
        }
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        for &(file, at, born) in &self.dropped {
            let time = (self.steps.done - born) as f32 / UPDATES_PER_SECOND;
            out.push(StrPiece {
                at: Some(at),
                ..StrPiece::new(file, time, false)
            });
        }
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.owner = pos;
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        self.sound.take()
    }

    fn ends_with_its_strs(&self) -> bool {
        self.steps.done > SCATTER_UPDATES
    }
}

/// Jupitel Thunderstorm (`CJupitelThunderStormEffect` 0x9c76b0): the ball flown from the caster to the target in 10
/// updates turned toward it, then bolts about the target at 80 ms apart, their count and spread by the skill's level
struct Jupitel {
    level: i32,
    ends: Ends,
    bolts: Vec<([f32; 3], &'static str, f32)>,
    steps: Steps,
    sounds: Vec<u32>,
}

const JUPITEL_BALL: &str =
    "HYPER_NOVICE/HN_JUPITEL_THUNDER_STORM/jupitel_thunder_storm_ball/jupitel_thunder_storm_ball";
const JUPITEL_BOLTS: [&str; 3] = [
    "HYPER_NOVICE/HN_JUPITEL_THUNDER_STORM/jupitel_thunder_storm/jupitel_thunder_storm_00",
    "HYPER_NOVICE/HN_JUPITEL_THUNDER_STORM/jupitel_thunder_storm/jupitel_thunder_storm_01",
    "HYPER_NOVICE/HN_JUPITEL_THUNDER_STORM/jupitel_thunder_storm/jupitel_thunder_storm_02",
];
const JUPITEL_SOUND: &str = "effect\\HYPER_NOVICE\\hu_jupitel_thunder_storm.wav";
const JUPITEL_FLIGHT: f32 = 10.0;

impl Jupitel {
    /// The bolts, once the target is known: bolt i at 80(i+1) ms while not past the level's last, ±r about the
    /// target in the order of i % 4's signs, 15 up
    fn lay_bolts(&mut self, target: [f32; 3]) {
        let level = self.level.max(1);
        let last = ((level - 1) / 2 + 6) * 150;
        let r = (((level - 1) / 4) * 10 + 25) / 2;
        const SIGNS: [(f32, f32); 4] = [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
        let count = (last / 80) as usize;
        self.bolts.reserve_exact(count);
        self.sounds.reserve_exact(count.div_ceil(4));
        for i in 0.. {
            let delay = 80 * (i + 1);
            if delay > last {
                break;
            }
            let (a, b, v) = (rand() % r, rand() % r, rand() % 3);
            let (sx, sz) = SIGNS[i as usize % 4];
            let at = [
                target[0] + sx * (b as f32 + 0.5),
                target[1] - 15.0,
                target[2] + sz * (a as f32 + 0.5),
            ];
            self.bolts
                .push((at, JUPITEL_BOLTS[v as usize], delay as f32 / 1000.0));
            if i % 4 == 0 {
                self.sounds.push(delay as u32 / 16 + 1);
            }
        }
    }
}

impl Effect for Jupitel {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        self.steps.held()
    }

    fn collect_draws(&self, _out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {}

    fn str_pieces(&self, out: &mut Vec<StrPiece>) {
        let (Some(from), Some(to)) = (self.ends.caster_at_start, self.ends.target_at_start) else {
            return;
        };
        let flown = (self.steps.done as f32 / JUPITEL_FLIGHT).min(1.0);
        out.push(StrPiece {
            at: Some(add(from, scale(sub(to, from), flown))),
            turn: StrTurn::Toward {
                point: to,
                plus: 0.0,
            },
            ..StrPiece::new(JUPITEL_BALL, self.steps.secs(), true)
        });
        let now = self.steps.secs();
        for &(at, file, delay) in &self.bolts {
            if now >= delay {
                out.push(StrPiece {
                    at: Some(at),
                    ..StrPiece::new(file, now - delay, false)
                });
            }
        }
    }

    fn set_link_endpoints(&mut self, caster: [f32; 3], target: [f32; 3]) {
        if self.ends.caster_at_start.is_none() {
            self.lay_bolts(target);
        }
        self.ends.set(caster, target);
    }

    fn take_sfx_request(&mut self) -> Option<&'static str> {
        let due = self.sounds.iter().position(|&at| at <= self.steps.done)?;
        self.sounds.swap_remove(due);
        Some(JUPITEL_SOUND)
    }
}

/// An aimed class: its STRs, how far up they play, the turn past the aim, its sound
fn aimed(
    files: &'static [(&'static str, bool)],
    lift: f32,
    plus: f32,
    sound: Option<&'static str>,
) -> Box<dyn Effect> {
    follow(files, lift, Some(plus), sound)
}

fn follow(
    files: &'static [(&'static str, bool)],
    lift: f32,
    aim: Option<f32>,
    sound: Option<&'static str>,
) -> Box<dyn Effect> {
    Box::new(Follow {
        files,
        lift,
        aim,
        sound,
        owner: [0.0; 3],
        target: None,
        steps: Steps::default(),
    })
}

/// The ring of four lines a Kagerou's or Oboro's cast sweeps (`CRotateLineEffect` 0x9b5060, render 0x9b52d0):
/// line L starts at 90L, 28 − 5L up, 7 − L out (3 more for Huuma Shuriken Construct's blue one), each a band of 21
/// quads of `cloud11.tga` that grows and turns for 60 updates; drawn the same at any turn of the camera
struct RotateLine {
    colour: [f32; 3],
    blend: BlendKind,
    widen: f32,
    /// Each line's band tilt, `80 + rand()%21` degrees
    tilts: [f32; 4],
    position: [f32; 3],
    steps: Steps,
}

const ROTATE_LINE_UPDATES: u32 = 60;
const ROTATE_LINE_SEGMENTS: u32 = 21;
const ROTATE_LINE_TEXTURE: &str = "cloud11.tga";
/// The textures the classes draw themselves (their STRs load their own)
pub const TEXTURES: &[&str] = &[ROTATE_LINE_TEXTURE];

impl RotateLine {
    fn new(colour: [f32; 3], blend: BlendKind, widen: f32) -> Self {
        Self {
            colour,
            blend,
            widen,
            tilts: std::array::from_fn(|_| (80 + rand() % 21) as f32),
            position: [0.0; 3],
            steps: Steps::default(),
        }
    }

    /// Segment k of line `line` at update n: its inner edge A and outer edge B, and its alpha out of 255
    fn edge(&self, line: u32, n: u32, k: u32, camera_turn: f32) -> ([f32; 3], [f32; 3], f32) {
        let start = (90 * line + 5 * (n + 1) + 5 * k) % 360;
        let span = (30 + 3 * (n + 1) + 3 * k).min(180);
        let angle = ((span / ROTATE_LINE_SEGMENTS) * k + start) % 360;
        let mut r = 7.0 - line as f32 + self.widen;
        if n >= 13 {
            r += 0.1 * (n as f32 - 12.0) + 0.1 * k as f32;
        }
        let alpha = match n {
            0..13 => 51 + n + k,
            13..=50 => 63 + k,
            // max(0, 63 − (n − 50) − k)
            _ => (63 + 50_u32).saturating_sub(n + k),
        } as f32;
        let (sin, cos) = (angle as f32 - camera_turn).to_radians().sin_cos();
        let h = 28.0 - 5.0 * line as f32;
        let (tilt_sin, tilt_cos) = self.tilts[line as usize].to_radians().sin_cos();
        let inner = [r * cos, -h, r * sin];
        let outer = add(inner, [tilt_cos * cos, 3.0 - tilt_sin, tilt_cos * sin]);
        (add(self.position, inner), add(self.position, outer), alpha)
    }
}

impl Effect for RotateLine {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.steps.advance(ctx.delta);
        if self.steps.done < ROTATE_LINE_UPDATES {
            EffectStatus::Running
        } else {
            EffectStatus::Dead
        }
    }

    fn collect_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        let n = self.steps.done.min(ROTATE_LINE_UPDATES - 1);
        // the ring is set back by the camera's turn (view+0x30)
        let camera_turn = ctx.camera.turn_degrees();
        let [r, g, b] = self.colour;
        for line in 0..4 {
            let mut previous = self.edge(line, n, 0, camera_turn);
            // kRO's first segment keeps its default corners, a sliver: left out
            for k in 1..ROTATE_LINE_SEGMENTS {
                let (inner, outer, alpha) = self.edge(line, n, k, camera_turn);
                let (u0, u1) = (
                    (k - 1) as f32 / ROTATE_LINE_SEGMENTS as f32,
                    k as f32 / ROTATE_LINE_SEGMENTS as f32,
                );
                out.push(EffectPrimitiveDraw::WorldQuad {
                    corners: [previous.1, outer, inner, previous.0],
                    uv: [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]],
                    texture: ROTATE_LINE_TEXTURE,
                    color: [r, g, b, alpha / 255.0],
                    blend: self.blend,
                    no_depth: false,
                });
                previous = (inner, outer, alpha);
            }
        }
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.position = pos;
    }
}

/// The level a class that reads its skill's takes: Gale Storm's and Jupitel Thunderstorm's top
const TOP_LEVEL: u8 = 10;

/// Gale Storm's `n` by its level (0x9d40a0): 5, 7, 9 and 11 from levels 1, 5, 8 and 10
fn gale_width(level: u8) -> i32 {
    match level {
        0..=4 => 5,
        5..=7 => 7,
        8..=9 => 9,
        _ => 11,
    }
}

fn scatter(kind: ScatterKind, n: i32) -> Box<dyn Effect> {
    Box::new(Scatter {
        kind,
        n,
        owner: [0.0; 3],
        dropped: Vec::with_capacity(kind.drops(n)),
        sound: None,
        steps: Steps::default(),
    })
}

fn flown(
    file: &'static str,
    per_step: f32,
    updates: f32,
    plus: f32,
    delay_updates: u32,
) -> Box<dyn Effect> {
    Box::new(Flown {
        file,
        per_step,
        updates,
        plus,
        delay_updates,
        ends: Ends::default(),
        steps: Steps::default(),
    })
}

fn boomerang(file: &'static str) -> Box<dyn Effect> {
    Box::new(Boomerang {
        file,
        ends: Ends::default(),
        t: 1.0,
        back: false,
        steps: Steps::default(),
    })
}

fn random_hit(file: &'static str) -> Box<dyn Effect> {
    let next = |turn: u8| Some((turn + 1) % RANDOM_HITS.len() as u8);
    let place = RANDOM_HIT_TURN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, next)
        .unwrap_or(0) as usize;
    Box::new(RandomHit {
        file,
        place,
        position: [0.0; 3],
        steps: Steps::default(),
    })
}

fn quake(
    files: &'static [(&'static str, Option<&'static str>)],
    sound: &'static str,
) -> Box<dyn Effect> {
    Box::new(Quake {
        files,
        sound: Some(sound),
        choice: None,
        steps: Steps::default(),
    })
}

fn acidified(element: &'static AcidElement) -> Box<dyn Effect> {
    Box::new(Acidified {
        element,
        ends: Ends::default(),
        target_left: Cell::new(None),
        steps: Steps::default(),
        sounds: [None; 2],
    })
}

/// A ported class: its id, whether it is made with the caster and the target (a link: the classes that fly from
/// one to the other or aim at the target), and how it is made
struct Class {
    id: EffectId,
    needs_target: bool,
    make: fn() -> Box<dyn Effect>,
}

use EffectId as E;

/// Every class `make` ports
const CLASSES: &[Class] = &[
    Class {
        id: E::Kro1756,
        needs_target: true,
        make: || {
            aimed(
                &[("crimsonarrow/crimsonarrow/crimsonarrow", false)],
                -13.0,
                0.0,
                Some("effect\\AG_CRIMSON_ARROW1.wav"),
            )
        },
    },
    Class {
        id: E::Kro2114,
        needs_target: true,
        make: || {
            aimed(
                &[("frenzy_shot/frenzy_shot/frenzy_shot", false)],
                -4.0,
                0.0,
                Some("effect\\ABC_FRENZY_SHOT.wav"),
            )
        },
    },
    Class {
        id: E::Kro2074,
        needs_target: true,
        make: || {
            aimed(
                &[(
                    "rhythmshooting/rhythmshooting_shot/rhythmshooting_shot",
                    false,
                )],
                -8.0,
                180.0,
                Some("effect\\TR_RHYTHMSHOOTING.wav"),
            )
        },
    },
    Class {
        id: E::Kro2154,
        needs_target: true,
        make: || {
            aimed(
                &[("unlucky_rush/unlucky_rush/unlucky_rush", false)],
                0.0,
                0.0,
                None,
            )
        },
    },
    Class {
        id: E::Kro2195,
        needs_target: true,
        make: || {
            aimed(
                &[(
                    "judgement_cross/judgement_cross_shot/judgement_cross_shot",
                    false,
                )],
                0.0,
                180.0,
                None,
            )
        },
    },
    Class {
        id: E::Kro1951,
        needs_target: true,
        make: || {
            aimed(
                &[("crescivebolt/crescivebolt/crescivebolt", false)],
                -3.0,
                180.0,
                Some("effect\\WH_CRESCIVE_BOLT.wav"),
            )
        },
    },
    // the Triangle Shot's start delay reads the caster's +0x58, not known to ours: none
    Class {
        id: E::Kro1324,
        needs_target: true,
        make: || {
            flown(
                "new_triangleshot/new_triangleshot_arrow/new_triangleshot_arrow",
                0.125,
                8.0,
                0.0,
                0,
            )
        },
    },
    Class {
        id: E::Kro1357,
        needs_target: true,
        make: || {
            flown(
                "new_cart_cannon/new_cart_cannon/new_cart_cannon",
                0.1,
                10.0,
                270.0,
                12,
            )
        },
    },
    Class {
        id: E::Kro1755,
        needs_target: true,
        make: || {
            Box::new(StormCannon {
                ends: Ends::default(),
                steps: Steps::default(),
                sound: Some("effect\\AG_STORM_CANNON.wav"),
            })
        },
    },
    Class {
        id: E::Kro1657,
        needs_target: true,
        make: || boomerang("new_axeboomerang/new_axeboomerang/new_axeboomerang"),
    },
    Class {
        id: E::Kro2183,
        needs_target: true,
        make: || boomerang("shield_shooting/shield_shooting/shield_shooting"),
    },
    Class {
        id: E::Kro2080,
        needs_target: false,
        make: || random_hit("4ig_secondfalth/secondfalth_hit/secondfalth_hit"),
    },
    Class {
        id: E::Kro2082,
        needs_target: false,
        make: || random_hit("4iq_secondjudgement/secondjudgement/secondjudgement"),
    },
    Class {
        id: E::Kro1760,
        needs_target: false,
        make: || quake(VIOLENT_QUAKE, "effect\\AG_VIOLENT_QUAKE.wav"),
    },
    Class {
        id: E::Kro1798,
        needs_target: false,
        make: || quake(ALL_BLOOM, "effect\\AG_ALL_BLOOM.wav"),
    },
    Class {
        id: E::Kro1837,
        needs_target: true,
        make: || {
            aimed(
                &[("savage_impact/savage_impact/savage_impact", false)],
                -12.0,
                0.0,
                Some("effect\\SHC_SAVAGE_IMPACT.wav"),
            )
        },
    },
    Class {
        id: E::Kro1911,
        needs_target: false,
        make: || {
            follow(
                &[
                    ("petitio/petitio_bottom/petitio_bottom", true),
                    ("petitio/petitio/petitio", false),
                ],
                -12.0,
                None,
                Some("effect\\CD_PETITIO.wav"),
            )
        },
    },
    Class {
        id: E::Kro1912,
        needs_target: false,
        make: || {
            follow(
                &[
                    (
                        "eternal_slash/eternal_slash_bottom/eternal_slash_bottom",
                        true,
                    ),
                    ("eternal_slash/eternal_slash/eternal_slash", false),
                ],
                -13.0,
                None,
                None,
            )
        },
    },
    Class {
        id: E::Kro1973,
        needs_target: false,
        make: || scatter(ScatterKind::GaleStorm, gale_width(TOP_LEVEL)),
    },
    Class {
        id: E::Kro2026,
        needs_target: false,
        make: || scatter(ScatterKind::DiamondStorm, 0),
    },
    Class {
        id: E::Kro2366,
        needs_target: true,
        make: || {
            Box::new(Jupitel {
                level: i32::from(TOP_LEVEL),
                ends: Ends::default(),
                bolts: Vec::new(),
                steps: Steps::default(),
                sounds: Vec::new(),
            })
        },
    },
    Class {
        id: E::RotateLineGray,
        needs_target: false,
        make: || Box::new(RotateLine::new([50.0 / 255.0; 3], BlendKind::Alpha, 0.0)),
    },
    Class {
        id: E::RotateLineBlue,
        needs_target: false,
        make: || {
            Box::new(RotateLine::new(
                [100.0 / 255.0, 100.0 / 255.0, 1.0],
                BlendKind::Additive,
                3.0,
            ))
        },
    },
    Class {
        id: E::Kro2206,
        needs_target: true,
        make: || acidified(&ACID_FIRE),
    },
    Class {
        id: E::Kro2214,
        needs_target: true,
        make: || acidified(&ACID_WATER),
    },
    Class {
        id: E::Kro2222,
        needs_target: true,
        make: || acidified(&ACID_WIND),
    },
    Class {
        id: E::Kro2230,
        needs_target: true,
        make: || acidified(&ACID_GROUND),
    },
];

fn class(id: EffectId) -> Option<&'static Class> {
    CLASSES.iter().find(|c| c.id == id)
}

/// The class ported for `id`, made at `at` (a cell's or a unit's place), or none. A class that reads the skill's
/// level (Gale Storm's spread, Jupitel Thunderstorm's bolts) takes the skill's top: the spawn carries no level
pub fn make(id: EffectId, at: [f32; 3]) -> Option<Box<dyn Effect>> {
    let mut made = (class(id)?.make)();
    made.set_position(at);
    Some(made)
}

/// Whether `make` ports `id`: its effect holds until it ends itself
pub fn ports(id: EffectId) -> bool {
    class(id).is_some()
}

/// Whether the class ported for `id` is made with the caster and the target (a link)
pub fn needs_target(id: EffectId) -> bool {
    class(id).is_some_and(|c| c.needs_target)
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

    fn pieces(effect: &dyn Effect) -> Vec<StrPiece> {
        let mut out = Vec::new();
        effect.str_pieces(&mut out);
        out
    }

    #[test]
    fn every_ported_class_is_made_and_draws() {
        for id in CLASSES.iter().map(|c| c.id) {
            let mut effect = make(id, [0.0; 3]).unwrap_or_else(|| panic!("{id:?} made"));
            effect.set_link_endpoints([0.0; 3], [50.0, 0.0, 0.0]);
            effect.set_position([0.0; 3]);
            effect.update(&ctx(0.5));
            let mut quads = EffectDrawList::new();
            effect.collect_draws(&mut quads, &render_ctx());
            assert!(
                !pieces(effect.as_ref()).is_empty() || !quads.primitives.is_empty(),
                "{id:?} draws"
            );
        }
    }

    /// A class draws only what ours draws as nothing: a look of our own for one of its ids must take it out of
    /// `CLASSES`, or the class would replace it
    #[test]
    fn every_class_is_one_of_our_no_ops() {
        for id in CLASSES.iter().map(|c| c.id) {
            assert!(
                matches!(
                    crate::table::own_effect_spec(id),
                    None | Some(crate::EffectSpec::Noop)
                ),
                "{id:?} has a look of ours"
            );
        }
    }

    #[test]
    fn a_quake_picks_the_same_file_at_the_same_cell() {
        let file_at = |at: [f32; 3]| pieces(make(EffectId::Kro1760, at).unwrap().as_ref())[0].file;
        assert_eq!(file_at([15.0, 0.0, 25.0]), file_at([15.0, 0.0, 25.0]));
    }

    #[test]
    fn a_flown_arrow_takes_longer_steps_on_a_short_way_and_ends_at_its_target() {
        let updates_to_end = |target_x: f32| {
            let mut arrow = make(EffectId::Kro1324, [0.0; 3]).unwrap();
            arrow.set_link_endpoints([0.0; 3], [target_x, 0.0, 0.0]);
            (1..100)
                .find(|_| arrow.update(&ctx(1.0 / 60.0)) == EffectStatus::Dead)
                .unwrap()
        };
        // 8 updates on a way of 40 or more; a way of 20 takes steps twice as long, half as many
        assert_eq!(updates_to_end(80.0), 9);
        assert_eq!(updates_to_end(20.0), 5);
    }

    fn render_ctx() -> EffectRenderCtx {
        EffectRenderCtx {
            camera: crate::effect_trait::CameraView {
                eye: [0.0, -50.0, -50.0],
                target: [0.0; 3],
                up: [0.0, -1.0, 0.0],
            },
            screen_w: 800.0,
            screen_h: 600.0,
            elapsed: 0.0,
        }
    }

    #[test]
    fn an_aimed_shot_follows_its_caster_turned_toward_where_the_target_stood() {
        let mut shot = make(EffectId::Kro2114, [0.0; 3]).unwrap();
        shot.set_link_endpoints([0.0; 3], [30.0, 0.0, 0.0]);
        shot.set_link_endpoints([5.0, 0.0, 0.0], [99.0, 0.0, 0.0]);
        let [piece] = pieces(shot.as_ref())[..] else {
            panic!("one STR");
        };
        assert_eq!(piece.at, Some([5.0, -4.0, 0.0]), "on the caster, 4 up");
        assert_eq!(
            piece.turn,
            StrTurn::Toward {
                point: [30.0, 0.0, 0.0],
                plus: 0.0
            }
        );
        assert_eq!(shot.take_sfx_request(), Some("effect\\ABC_FRENZY_SHOT.wav"));
        assert_eq!(shot.take_sfx_request(), None, "once");
    }

    #[test]
    fn a_cannon_ball_rolls_to_its_target_then_fades() {
        let mut ball = make(EffectId::Kro1755, [0.0; 3]).unwrap();
        ball.set_link_endpoints([0.0; 3], [17.0, 0.0, 0.0]);
        ball.update(&ctx(5.0 / 60.0));
        let [shadow, cannon] = pieces(ball.as_ref())[..] else {
            panic!("two STRs");
        };
        assert!(shadow.under_sprites && !cannon.under_sprites);
        let at = cannon.at.unwrap();
        assert!((at[0] - (8.5 - 1.5)).abs() < 1e-4, "1.7 a update: {at:?}");
        assert_eq!(cannon.alpha, 1.0);
        ball.update(&ctx(10.0 / 60.0));
        assert!(
            pieces(ball.as_ref())[0].alpha < 1.0,
            "fading past its target"
        );
        ball.update(&ctx(30.0 / 60.0));
        assert_eq!(ball.update(&ctx(0.0)), EffectStatus::Dead);
    }

    #[test]
    fn a_boomerang_goes_out_and_comes_back_in_about_72_updates() {
        let mut axe = make(EffectId::Kro1657, [0.0; 3]).unwrap();
        axe.set_link_endpoints([0.0; 3], [40.0, 0.0, 0.0]);
        let mut updates = 0;
        while axe.update(&ctx(1.0 / 60.0)) == EffectStatus::Running && updates < 200 {
            updates += 1;
        }
        assert!((70..=74).contains(&updates), "{updates}");
    }

    #[test]
    fn a_bottle_flies_until_it_lands_then_bursts_with_its_sounds() {
        let mut acid = make(EffectId::Kro2206, [0.0; 3]).unwrap();
        acid.set_link_endpoints([0.0; 3], [40.0, 0.0, 0.0]);
        acid.update(&ctx(1.0 / 60.0));
        assert!(
            pieces(acid.as_ref()).is_empty(),
            "no side known before a draw"
        );
        acid.collect_draws(&mut EffectDrawList::new(), &render_ctx());
        assert_eq!(pieces(acid.as_ref()).len(), 1, "the bottle");
        assert_eq!(acid.take_sfx_request(), None);
        acid.update(&ctx(13.0 / 60.0));
        assert_eq!(pieces(acid.as_ref()).len(), 3, "the burst and its floor");
        assert_eq!(acid.take_sfx_request(), Some(ACID_SOUND));
        assert_eq!(
            acid.take_sfx_request(),
            Some("effect\\BO_ACIDIFIED_ZONE_FIRE.wav")
        );
    }

    #[test]
    fn a_bottle_bends_the_same_way_on_screen_whichever_side_its_target_stands() {
        let bottle_at = |target_x: f32| {
            let mut acid = make(EffectId::Kro2206, [0.0; 3]).unwrap();
            acid.set_link_endpoints([0.0; 3], [target_x, 0.0, 0.0]);
            acid.update(&ctx(3.0 / 60.0));
            acid.collect_draws(&mut EffectDrawList::new(), &render_ctx());
            pieces(acid.as_ref())[0].at.unwrap()
        };
        let (right, left) = (bottle_at(40.0), bottle_at(-40.0));
        assert!(
            (right[0] + left[0]).abs() < 1e-3,
            "{right:?} {left:?} mirrored"
        );
        assert!(
            (right[2] - left[2]).abs() < 1e-3 && right[2] != 0.0,
            "bent alike"
        );
    }

    #[test]
    fn petitio_follows_its_target_12_up_its_floor_under_the_units() {
        let mut petitio = make(EffectId::Kro1911, [0.0; 3]).unwrap();
        petitio.set_position([1.0, 0.0, 1.0]);
        let [floor, top] = pieces(petitio.as_ref())[..] else {
            panic!("two STRs");
        };
        assert!(floor.under_sprites && !top.under_sprites);
        assert_eq!(top.at, Some([1.0, -12.0, 1.0]));
        assert_eq!(top.turn, StrTurn::None, "not aimed");
        assert_eq!(petitio.take_sfx_request(), Some("effect\\CD_PETITIO.wav"));
    }

    #[test]
    fn storms_drop_on_their_schedule_within_their_spread() {
        let mut diamond = make(EffectId::Kro2026, [100.0, 0.0, 100.0]).unwrap();
        diamond.update(&ctx(61.0 / 60.0));
        let drops = pieces(diamond.as_ref());
        assert_eq!(drops.len(), 13, "every 5 updates to 60");
        assert!(drops.iter().all(|p| {
            let [x, _, z] = p.at.unwrap();
            (x - 100.0).abs() <= 22.5 && (z - 100.0).abs() <= 22.5
        }));
        let mut gale = make(EffectId::Kro1973, [0.0; 3]).unwrap();
        gale.update(&ctx(61.0 / 60.0));
        let drops = pieces(gale.as_ref());
        let storms = drops.iter().filter(|p| p.file == GALE_STORM).count();
        assert_eq!(
            (storms, drops.len() - storms),
            (7, 31),
            "n 11: every 9 and every 2 updates"
        );
    }

    #[test]
    fn jupitel_flies_its_ball_then_drops_its_bolts_80_ms_apart() {
        let mut storm = make(EffectId::Kro2366, [0.0; 3]).unwrap();
        storm.set_link_endpoints([0.0; 3], [100.0, 0.0, 0.0]);
        storm.update(&ctx(5.0 / 60.0));
        let flying = pieces(storm.as_ref());
        assert_eq!(flying.len(), 2, "the ball and the first bolt");
        assert_eq!(
            flying[0].at,
            Some([50.0, 0.0, 0.0]),
            "half its way in 5 of 10 updates"
        );
        storm.update(&ctx(2.0));
        assert_eq!(
            pieces(storm.as_ref()).len(),
            1 + 18,
            "level 10: bolts to 1440 ms"
        );
        assert_eq!(storm.take_sfx_request(), Some(JUPITEL_SOUND));
    }

    #[test]
    fn a_rotate_line_sweeps_four_bands_of_20_quads_for_60_updates() {
        let mut ring = make(EffectId::RotateLineGray, [0.0; 3]).unwrap();
        let mut draws = EffectDrawList::new();
        ring.collect_draws(&mut draws, &render_ctx());
        assert_eq!(draws.primitives.len(), 4 * 20);
        assert_eq!(ring.update(&ctx(59.0 / 60.0)), EffectStatus::Running);
        assert_eq!(ring.update(&ctx(1.0 / 60.0)), EffectStatus::Dead);
    }

    #[test]
    fn a_random_hit_plays_at_one_of_its_three_places_with_its_turn() {
        let mut hit = make(EffectId::Kro2082, [0.0; 3]).unwrap();
        hit.set_position([10.0, 0.0, 10.0]);
        hit.update(&ctx(0.5));
        let [piece] = pieces(hit.as_ref())[..] else {
            panic!("one STR, past any of the delays");
        };
        let place = RANDOM_HITS
            .iter()
            .find(|(offset, _, _)| piece.at == Some(add([10.0, 0.0, 10.0], *offset)))
            .expect("one of the three places");
        assert_eq!(piece.turn, StrTurn::Fixed(place.1));
    }
}
