//! kRO's effect primitives (`CEffectPrim`, vtable 0xd2a590), ported from the 2022 exe's code: each type's update and
//! render as the exe runs them (`docs/nostalre-kro-exe-effect-types.md`, the decompiled functions named below), on a
//! primitive kept as the exe lays it out (its fields read and written by their offsets), set up by the exe's own
//! values (`generated.rs`, `scripts/kro-exe/prim_recipes.py`: every field once the effect's code has set it up).
//! An effect is replayed, not re-imagined: its primitives made at the updates kRO makes them, stepped every 16 ms
//! as kRO's world steps its objects, drawn as the quads kRO hands its renderer.

mod generated;
mod types;

pub use generated::TEXTURES;

use models::enums::EnumWithNumberValue;
use models::enums::effect_id::EffectId;

use crate::draw::{BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect_trait::{Effect, EffectRenderCtx, EffectUpdateCtx};
use crate::vec3::add;

/// One of kRO's world steps (0x7796c8 counts them in 16 ms of real time).
const STEP: f32 = 0.016;
/// A primitive's fields below its vertex slots (+0x2210 on), which the port does not keep.
const SIZE: usize = 0x2210;

pub(crate) struct EffectRecipe {
    pub id: u16,
    /// The effect's own length (+0x17c, as its code leaves it: the update kRO's run ended it, plus one): in the
    /// update it reaches it, it ends and its primitives with it (their owner gone, 0x68e550 ends them) before they
    /// are drawn. -1: none read.
    pub length: i32,
    /// The effect's colour overrides the quad emitters read (+0x11c88 red, +0x11c8c green, +0x11c90 blue, +0x11c94
    /// destination blend); all ones: none.
    pub owner_colour: [u32; 4],
    pub prims: &'static [PrimRecipe],
}

pub(crate) struct PrimRecipe {
    pub kind: u16,
    /// The update it is made in (-1: with the effect).
    pub at: i32,
    pub fields: &'static [(u16, u32)],
    pub textures: &'static [&'static str],
}

/// The exe's recipe for `id` (`RECIPES` is in id order): the table holds only the effects the port draws, each
/// a no-op of ours (`prim_recipe_table.py`'s `WANTED`; signed-off looks are not replaced here)
fn recipe(id: EffectId) -> Option<&'static EffectRecipe> {
    let at = generated::RECIPES
        .binary_search_by_key(&id.value(), |r| usize::from(r.id))
        .ok()?;
    Some(&generated::RECIPES[at])
}

/// Whether `id` is drawn by the port.
pub fn ports(id: EffectId) -> bool {
    recipe(id).is_some()
}

/// The ported effect for `id` at `pos`, if the port draws it.
pub fn effect(id: EffectId, pos: [f32; 3]) -> Option<Box<dyn Effect>> {
    Some(Box::new(KroEffect::new(recipe(id)?, pos)))
}

/// kRO's cosine of whole degrees (0x4dd6f0): past ±360 the C remainder, a negative angle read from the table's end
/// (entry 361 + k: −1° gives cos 360°, the exe's own off-by-one).
pub fn cos(k: i32) -> f32 {
    lookup(&generated::COS, k)
}

/// kRO's sine of whole degrees (0x4dd790), as [`cos`].
pub fn sin(k: i32) -> f32 {
    lookup(&generated::SIN, k)
}

/// kRO's whole degrees in radians (0x4dd740), as [`cos`].
pub(crate) fn rad(k: i32) -> f32 {
    lookup(&generated::RAD, k)
}

fn lookup(table: &[u32; 361], k: i32) -> f32 {
    let k = if !(-360..=360).contains(&k) {
        k % 360
    } else {
        k
    };
    f32::from_bits(if k < 0 {
        table[(361 + k) as usize]
    } else {
        table[k as usize]
    })
}

/// A table read as the render code reads it, by its index directly (0 to 360). kRO reads past the table for an index
/// out of it; no recipe replayed gets there (the check against the exe's draws would show it), and here it is held
/// at the table's ends rather than read out of bounds.
pub(crate) fn cos_at(i: i32) -> f32 {
    debug_assert!((0..=360).contains(&i), "angle {i} outside kRO's table");
    f32::from_bits(generated::COS[i.clamp(0, 360) as usize])
}

pub(crate) fn sin_at(i: i32) -> f32 {
    debug_assert!((0..=360).contains(&i), "angle {i} outside kRO's table");
    f32::from_bits(generated::SIN[i.clamp(0, 360) as usize])
}

/// The effect a primitive belongs to, as its updates and emitters read it.
pub(crate) struct Owner {
    pub pos: [f32; 3],
    pub colour: [u32; 4],
    /// Its length +0x17c and its count +0x178 (its own update, before its primitives', counts it up: the updates
    /// made so far, this one included).
    pub length: i32,
    pub count: i32,
}

impl Owner {
    /// Its red, green and blue overrides (+0x11c88 to +0x11c90) over `rgb`; all ones: none.
    fn tint(&self, (r, g, b): (u32, u32, u32)) -> (u32, u32, u32) {
        let over = |o: u32, c| if o != u32::MAX { o & 0xff } else { c };
        (
            over(self.colour[0], r),
            over(self.colour[1], g),
            over(self.colour[2], b),
        )
    }

    /// Its destination blend override (+0x11c94) over `dest`.
    fn dest(&self, dest: u32) -> u32 {
        if self.colour[3] != u32::MAX {
            self.colour[3]
        } else {
            dest
        }
    }
}

/// One kRO primitive: its fields as the exe lays them out.
pub(crate) struct Prim {
    mem: Box<[u8; SIZE]>,
    pub textures: &'static [&'static str],
}

impl Prim {
    fn new(recipe: &'static PrimRecipe) -> Self {
        let mut mem = Box::new([0u8; SIZE]);
        for &(off, value) in recipe.fields {
            mem[usize::from(off)..usize::from(off) + 4].copy_from_slice(&value.to_le_bytes());
        }
        let prim = Prim {
            mem,
            textures: recipe.textures,
        };
        debug_assert_eq!(
            prim.kind(),
            u32::from(recipe.kind),
            "the recipe's type is its +0x1d0"
        );
        prim
    }

    pub fn f(&self, off: usize) -> f32 {
        f32::from_le_bytes(self.mem[off..off + 4].try_into().unwrap())
    }
    pub fn set_f(&mut self, off: usize, v: f32) {
        self.mem[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn i(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.mem[off..off + 4].try_into().unwrap())
    }
    pub fn set_i(&mut self, off: usize, v: i32) {
        self.mem[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn s(&self, off: usize) -> i16 {
        i16::from_le_bytes(self.mem[off..off + 2].try_into().unwrap())
    }
    pub fn set_s(&mut self, off: usize, v: i16) {
        self.mem[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn u8(&self, off: usize) -> u8 {
        self.mem[off]
    }
    pub fn set_u8(&mut self, off: usize, v: u8) {
        self.mem[off] = v;
    }
    pub fn kind(&self) -> u32 {
        self.i(0x1d0) as u32
    }
    pub fn pos(&self) -> [f32; 3] {
        [self.f(0x10), self.f(0x14), self.f(0x18)]
    }
    pub fn copy_owner_pos(&mut self, owner: &Owner) {
        self.set_f(0x10, owner.pos[0]);
        self.set_f(0x14, owner.pos[1]);
        self.set_f(0x18, owner.pos[2]);
    }
}

/// A quad as kRO hands its renderer: corners A, B, C, D and its two triangles (most (A, B, C) and (D, C, A); our
/// world quads cut (0, 1, 2), (0, 2, 3): the same two).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KroQuad {
    pub corners: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    /// Alpha, red, green, blue.
    pub argb: u32,
    /// D3DBLEND source and destination.
    pub blend: (u8, u8),
    pub texture: &'static str,
    /// Drawn in front of everything (0x6c3a10's mode 1).
    pub front: bool,
    /// Filed with flag 1 (the renderer's lists +0xd0 and +0xe8, 0x4e8570): drawn source alpha over inverse source
    /// alpha, back to front, whatever `blend` holds.
    pub sorted: bool,
    /// The two triangles kRO hands its renderer, as corner indices.
    pub tris: [[usize; 3]; 2],
}

/// The triangles of a quad drawn A, B, C, D: (A, B, C) and (D, C, A).
const QUAD_TRIS: [[usize; 3]; 2] = [[0, 1, 2], [3, 2, 0]];

/// One primitive's render call: its owner, where its quads go, and the renderer's per-call count (+0x2210 slots: a
/// call stops past 0x1fd triangles).
pub(crate) struct Emitted<'a> {
    owner: &'a Owner,
    count: i32,
    sink: &'a mut dyn FnMut(KroQuad),
}

impl Emitted<'_> {
    fn full(&self) -> bool {
        self.count > 0x1fd
    }

    fn push(&mut self, q: KroQuad) {
        (self.sink)(q);
        self.count += 2;
    }
}

fn argb(alpha: i32, r: u32, g: u32, b: u32) -> u32 {
    (((alpha.wrapping_mul(0x100) as u32).wrapping_add(r))
        .wrapping_mul(0x100)
        .wrapping_add(g))
    .wrapping_mul(0x100)
    .wrapping_add(b)
}

/// 0x6c4430: one quad relative to the primitive, corners A, B, C, D, coloured by `preset` (the +0x318 it reads, the
/// table read from its switch at 0x6c448a), u along a ring's 21 segments, v from 1 at A and B to 0 at C and D.
pub(crate) fn emit_ring_quad(
    p: &Prim,
    e: &mut Emitted,
    preset: i32,
    seg: i32,
    alpha: i16,
    corners: [[f32; 3]; 4],
    tex: i32,
) {
    if e.full() {
        return;
    }
    let mut alpha = i32::from(alpha);
    let (rgb, dest, v1) = match preset {
        0 => (0xffffff, 6, 1.0),
        1 => (0xffafaf, 2, 1.0),
        2 => (0xc3c3ff, 6, 1.0),
        3 => {
            alpha = 0x50;
            (0xffffff, 6, 1.0)
        }
        4 => (0x6464ff, 2, 1.0),
        6 => (0xff6464, 2, 1.0),
        7 => (0x64ff64, 2, 1.0),
        8 => (0xffffaa, 2, 1.0),
        9 => (0x8cd28c, 2, 1.0),
        10 => (0xff59b6, 2, 1.0),
        11 => (0xaaaaff, 2, 1.0),
        12 => (0x323232, 6, 1.0),
        13 => (0xff32ff, 6, 1.0),
        14 => (0x7878ff, 6, 1.0),
        15 => (0x0aff0a, 2, 1.0),
        16 => (0x14ff14, 2, 1.0),
        17 => (0x0000ff, 2, 1.0),
        18 => (0xff0000, 2, 1.0),
        19 => (0x00ff00, 2, 1.0),
        20 => (0xffff00, 2, 1.0),
        21 => (0xff00ff, 2, 1.0),
        22 => (0x00ffff, 2, 1.0),
        23 => (0x295ebc, 6, 1.0),
        24 => (0x0a4272, 2, 1.0),
        25 => (0x2392b9, 2, 1.0),
        26 => (0xa4cdfe, 2, 1.0),
        27 => (0x54a9cd, 2, 1.0),
        28 => (0x6c1aff, 2, 1.0),
        29 => (0xb1ff3a, 2, 1.0),
        30 => (0x551eb9, 2, 1.0),
        31 => (0xf4bb7c, 2, 1.0),
        32 => (0x000000, 6, 1.0),
        40 => (0x4646ff, 2, 1.0),
        41 => (0x1e1eff, 2, 1.0),
        42 => (0x2b282e, 6, 1.0),
        43 => (0x7db6a0, 6, 1.0),
        44 => (0xb4371e, 6, 1.0),
        45 => (0x418de7, 2, 1.0),
        46 => (0xa42f1d, 2, 1.0),
        47 => (0xac9a38, 2, 1.0),
        48 => (0x501414, 2, 1.0),
        49 => (0xc88254, 2, 1.0),
        50 => (0xffc65b, 2, 1.0),
        100 => (0xfafaff, 6, f32::from_bits(0x3f66_6666)),
        _ => (0xffffff, 2, 1.0),
    };
    let (r, g, bl) = e
        .owner
        .tint(((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff));
    let dest = e.owner.dest(dest);
    let (u0, u1) = match seg {
        s if s < 100 => (
            if s == 0 { 0.0 } else { s as f32 / 21.0 },
            (s + 1) as f32 / 21.0,
        ),
        100 => (f32::from_bits(0x3eb3_3333), f32::from_bits(0x3ee6_6666)),
        101 => (f32::from_bits(0x3ee6_6666), f32::from_bits(0x3f0c_cccd)),
        _ => (f32::from_bits(0x3f0c_cccd), f32::from_bits(0x3f26_6666)),
    };
    let pos = p.pos();
    e.push(KroQuad {
        corners: corners.map(|c| add(pos, c)),
        uv: [[u0, v1], [u1, v1], [u1, 0.0], [u0, 0.0]],
        argb: argb(alpha, r, g, bl),
        blend: (5, dest as u8),
        texture: texture(p, tex),
        front: false,
        sorted: false,
        tris: QUAD_TRIS,
    });
}

/// 0x6c3a10: one quad of a given colour and uv; blend flag 10 draws one / zero, 99 alpha, others additive (flag 0)
/// or alpha (flag != 0); mode 1 in front of everything.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_quad(
    p: &Prim,
    e: &mut Emitted,
    alpha: i16,
    corners: [[f32; 3]; 4],
    flag: i32,
    tex: i32,
    rgb: (u32, u32, u32),
    uv: (f32, f32, f32, f32),
    mode: i32,
) {
    if e.full() {
        return;
    }
    let (r, g, bl) = e.owner.tint(rgb);
    let blend = match flag {
        10 => (2, 1),
        99 => (5, 6),
        _ => (5, e.owner.dest(u32::from(flag != 0) * 4 + 2) as u8),
    };
    let pos = p.pos();
    let (v0, v1, u0, u1) = uv;
    e.push(KroQuad {
        corners: corners.map(|c| add(pos, c)),
        uv: [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
        argb: argb(i32::from(alpha), r, g, bl),
        blend,
        texture: texture(p, tex),
        front: mode == 1,
        sorted: false,
        tris: QUAD_TRIS,
    });
}

/// 0x6a9ff0's and 0x6ad4b0's quad: points P0 to P3 (relative to the primitive), triangles (P0, P1, P2) and
/// (P3, P2, P1), uv P0 (0, v0), P1 (1, v0), P2 (0, v1), P3 (1, v1); coloured +0x1c0 (no owner overrides), texture
/// frame +0x3c, blend fields unset (the slots' zeros), filed in the sorted alpha lists (+0x1c4 1: the only filing
/// `prim_recipe_table.py` replays). Corners go P1, P3, P2, P0: a cycle,
/// its first diagonal kRO's.
pub(crate) fn emit_strip(p: &Prim, e: &mut Emitted, pts: [[f32; 3]; 4], (v0, v1): (f32, f32)) {
    let pos = p.pos();
    let [p0, p1, p2, p3] = pts;
    e.push(KroQuad {
        corners: [p1, p3, p2, p0].map(|c| add(pos, c)),
        uv: [[1.0, v0], [1.0, v1], [0.0, v1], [0.0, v0]],
        argb: p.i(0x1c0) as u32,
        blend: (0, 0),
        texture: texture(p, p.i(0x3c)),
        front: false,
        sorted: true,
        tris: [[3, 0, 2], [1, 2, 0]],
    });
}

/// The primitive's texture `tex` ("" past its list: none of the replayed recipes gets there).
fn texture(p: &Prim, tex: i32) -> &'static str {
    usize::try_from(tex)
        .ok()
        .and_then(|t| p.textures.get(t))
        .copied()
        .unwrap_or("")
}

/// The primitive's update (0x68e550): its type's motion, then its count +0x1e0 up by one; alive while the count is
/// at most its life +0x1e4.
fn update(p: &mut Prim, owner: &Owner) -> bool {
    types::update(p, owner);
    let count = p.i(0x1e0).wrapping_add(1);
    p.set_i(0x1e0, count);
    count <= p.i(0x1e4)
}

/// The quads a primitive draws this step (0x6addc0, while it is shown, +0xa0). kRO's renders write the primitive
/// (0x95 its preset +0x318, 0xa1 its corners) but nothing reads those writes back before setting them again, so
/// here they are passed along instead and the primitive is left as its update left it.
fn render(p: &Prim, owner: &Owner, sink: &mut dyn FnMut(KroQuad)) {
    if p.u8(0xa0) != 0 {
        types::render(
            p,
            &mut Emitted {
                owner,
                count: 0,
                sink,
            },
        );
    }
}

struct KroEffect {
    recipe: &'static EffectRecipe,
    owner: Owner,
    /// Its primitives in the order made, `None` once ended.
    prims: Vec<Option<Prim>>,
    made: usize,
    step: i32,
    pending: f32,
}

impl KroEffect {
    fn new(recipe: &'static EffectRecipe, pos: [f32; 3]) -> Self {
        KroEffect {
            recipe,
            owner: Owner {
                pos,
                colour: recipe.owner_colour,
                length: recipe.length,
                count: 0,
            },
            prims: Vec::new(),
            made: 0,
            step: 0,
            pending: 0.0,
        }
    }

    /// One world step: the primitives due made, then each updated in the order made; in the step the effect reaches
    /// its length all of them end.
    fn step(&mut self) {
        if self.recipe.length > 0 && self.step >= self.recipe.length - 1 {
            self.prims.clear();
            self.made = self.recipe.prims.len();
            self.step += 1;
            return;
        }
        self.owner.count = self.step + 1;
        while self.made < self.recipe.prims.len() && self.recipe.prims[self.made].at <= self.step {
            self.prims
                .push(Some(Prim::new(&self.recipe.prims[self.made])));
            self.made += 1;
        }
        for slot in &mut self.prims {
            if let Some(p) = slot
                && !update(p, &self.owner)
            {
                *slot = None;
            }
        }
        self.step += 1;
    }

    /// What every live primitive draws now, in the order made.
    fn each_quad(&self, sink: &mut dyn FnMut(KroQuad)) {
        for p in self.prims.iter().flatten() {
            render(p, &self.owner, sink);
        }
    }

    fn finished(&self) -> bool {
        self.made == self.recipe.prims.len() && self.prims.iter().all(Option::is_none)
    }
}

impl Effect for KroEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.pending += ctx.delta;
        while self.pending >= STEP && !self.finished() {
            self.pending -= STEP;
            self.step();
        }
        if self.finished() {
            EffectStatus::Dead
        } else {
            EffectStatus::Running
        }
    }

    fn collect_draws(&self, out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {
        self.each_quad(&mut |q| {
            let color = [
                ((q.argb >> 16) & 0xff) as f32 / 255.0,
                ((q.argb >> 8) & 0xff) as f32 / 255.0,
                (q.argb & 0xff) as f32 / 255.0,
                (q.argb >> 24) as f32 / 255.0,
            ];
            let blend = match q.blend {
                _ if q.sorted => BlendKind::Alpha,
                (5, 2) => BlendKind::Additive,
                (5, 6) => BlendKind::Alpha,
                (src, dst) => BlendKind::Raw {
                    src: i32::from(src),
                    dst: i32::from(dst),
                },
            };
            out.push(EffectPrimitiveDraw::WorldQuad {
                corners: q.corners,
                uv: q.uv,
                texture: q.texture,
                color,
                blend,
                no_depth: q.front,
            });
        });
    }

    fn set_position(&mut self, pos: [f32; 3]) {
        self.owner.pos = pos;
    }
}

/// Every update's quads of a ported effect from the origin, `updates` long, as the exe's runs record theirs (each
/// update's draws after its update pass): for checking the port against them (example `kro_prim_trace`).
pub fn trace(id: EffectId, updates: usize) -> Option<Vec<Vec<KroQuad>>> {
    let mut e = KroEffect::new(recipe(id)?, [0.0; 3]);
    let mut frames = Vec::new();
    for _ in 0..updates {
        if e.finished() {
            break;
        }
        e.step();
        let mut quads = Vec::new();
        e.each_quad(&mut |q| quads.push(q));
        frames.push(quads);
    }
    Some(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Effect 830's first triangle as the exe's run drew it (out/kro-effect-draws/files-plain/830.jsonl, update 0):
    /// its places to 3 decimals, colour, uv, texture and blend.
    #[test]
    fn beginspell_n1_draws_kro_first_triangle() {
        let frames = trace(EffectId::BeginspellN1, 1).unwrap();
        let q = frames[0][0];
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.001);
        assert!(close(q.corners[0], [-4.17, 0.0, 0.0]));
        assert!(close(q.corners[1], [-3.966, 0.0, -1.289]));
        assert!(close(q.corners[2], [-9.414, -29.473, -3.059]));
        assert_eq!(q.argb, 0xb164_64ff);
        assert_eq!(q.uv[0], [0.0, 1.0]);
        assert!((q.uv[1][0] - 0.0476).abs() < 0.0001);
        assert_eq!((q.texture, q.blend), ("W_bubble02.tga", (5, 2)));
    }

    /// kRO's first vertex of `id`'s run at update `at` (out/kro-effect-draws/files-plain), the quads drawn then, and
    /// the corner holding it.
    fn first_vertex(
        id: EffectId,
        at: usize,
        quads: usize,
        place: [f32; 3],
        argb: u32,
        uv: [f32; 2],
    ) -> KroQuad {
        let frames = trace(id, at + 1).unwrap();
        assert_eq!(frames[at].len(), quads, "{id:?} quads at update {at}");
        let q = frames[at][0];
        let c = q.tris[0][0];
        assert!(
            q.corners[c]
                .iter()
                .zip(place)
                .all(|(x, y)| (x - y).abs() < 0.001),
            "{id:?} first vertex {:?}",
            q.corners[c]
        );
        assert_eq!((q.argb, q.uv[c]), (argb, uv), "{id:?}");
        q
    }

    /// Type 0x10 (Agiup2, 777): two crossed quads a primitive, in kRO's sorted alpha list with the slots' unset
    /// blend, triangles (P0, P1, P2) and (P3, P2, P1).
    #[test]
    fn type_10_draws_crossed_quads_sorted() {
        let q = first_vertex(
            EffectId::Agiup2,
            0,
            2,
            [-0.35, -4.74, 3.0],
            0x03ff_ffff,
            [0.0, 1.0],
        );
        assert_eq!(
            (q.texture, q.blend, q.sorted),
            ("ac_center2.tga", (0, 0), true)
        );
        assert_eq!(q.tris, [[3, 0, 2], [1, 2, 0]]);
    }

    /// Type 0xf (Detect2, 725): one upright quad, still one at update 20.
    #[test]
    fn type_0f_draws_one_upright_quad() {
        let q = first_vertex(
            EffectId::Detect2,
            0,
            1,
            [1.607, -5.0, 1.607],
            0xfeff_ffff,
            [0.0, 0.0],
        );
        assert_eq!((q.texture, q.sorted), ("fashasha.tga", true));
        assert_eq!(trace(EffectId::Detect2, 21).unwrap()[20].len(), 1);
    }

    /// Type 0x58 (Stasis, 799): six primitives' squares at update 0; Groundshake's (894) drawn in front.
    #[test]
    fn type_58_draws_spreading_squares() {
        let q = first_vertex(
            EffectId::Stasis,
            0,
            6,
            [11.7, -10.0, 0.0],
            0x0037_37ff,
            [0.0, 1.0],
        );
        assert_eq!(q.texture, "thunder_ball_0001.bmp");
        let front = trace(EffectId::Groundshake, 51).unwrap();
        assert!(front[0].len() == 1 && front[50].len() == 1 && front[0][0].front);
    }

    /// Type 0x9f (Primecharge, 882): two quads a record, 4 at update 0, 320 at update 100.
    #[test]
    fn type_9f_draws_two_bands_a_record() {
        let q = first_vertex(
            EffectId::Primecharge,
            0,
            4,
            [-34.965, -73.078, -14.161],
            0x0500_fafa,
            [0.0, 1.0],
        );
        assert_eq!(q.texture, "alpha_center.tga");
        assert_eq!(trace(EffectId::Primecharge, 101).unwrap()[100].len(), 320);
    }

    /// Type 0x92 (HealN, 832): 21 quads at update 0, 84 at update 30, their texture the counter's frame.
    #[test]
    fn type_92_draws_drifting_walls() {
        let q = first_vertex(
            EffectId::HealN,
            0,
            21,
            [-2.148, 0.0, -0.776],
            0x19ff_ffff,
            [0.0, 1.0],
        );
        assert_eq!(q.texture, "W_bubble04.tga");
        assert_eq!(trace(EffectId::HealN, 31).unwrap()[30].len(), 84);
    }

    /// Each ported effect is drawn no later than the update before kRO's run ended it (check_prim_port.py holds
    /// the updates before).
    #[test]
    fn ported_effects_end_when_kro_ends_them() {
        for r in generated::RECIPES {
            let id = recipe_id(r);
            // a length kRO's run did not show (-1) holds nothing to check against
            let Ok(length) = usize::try_from(r.length) else {
                continue;
            };
            let frames = trace(id, 1000).unwrap();
            assert!(frames.len() <= length, "{id:?} runs past its end");
            assert!(
                frames.get(length - 1).is_none_or(Vec::is_empty),
                "{id:?} gone at its end"
            );
        }
    }

    /// Every quad a ported effect draws is in a blend the renderer has (additive or alpha): any other (`Raw`) is
    /// drawn as one of those, so a port needing one must not pass unseen.
    #[test]
    fn ported_effects_blend_as_the_renderer_draws() {
        for id in generated::RECIPES.iter().map(recipe_id) {
            for q in trace(id, 1000).unwrap().iter().flatten() {
                assert!(
                    q.sorted || matches!(q.blend, (5, 2) | (5, 6)),
                    "{id:?} blends {:?}",
                    q.blend
                );
            }
        }
    }

    fn recipe_id(r: &EffectRecipe) -> EffectId {
        EffectId::from_value(usize::from(r.id))
    }

    /// The table binary-searches.
    #[test]
    fn recipes_in_id_order() {
        assert!(generated::RECIPES.windows(2).all(|w| w[0].id < w[1].id));
    }

    /// The port draws only what ours draws as nothing (prim_recipe_table.py's `WANTED`): a look of our own given
    /// to one of its ids must take it out of `WANTED`, or the next table would replace it.
    #[test]
    fn the_port_draws_only_our_no_ops() {
        for id in generated::RECIPES.iter().map(recipe_id) {
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
    fn ported_effects_are_made_by_the_port() {
        for id in generated::RECIPES.iter().map(recipe_id) {
            assert!(
                matches!(crate::effect_spec(id), Some(crate::EffectSpec::Custom)),
                "{id:?}"
            );
            assert_eq!(crate::table::custom_duration_ms(id), u32::MAX, "{id:?}");
            assert!(
                crate::factory::make_effect(
                    id,
                    crate::spec::EffectAnchor::Point([0.0; 3]),
                    None,
                    None,
                    None
                )
                .is_some()
            );
        }
    }
}
