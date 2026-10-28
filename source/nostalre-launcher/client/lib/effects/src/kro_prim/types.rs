//! The primitive types the port has, each as the exe's update and render read (their addresses below). Records:
//! four of 0xb4 bytes from +0x3e8 (`rec`): +0 byte active, +2 short alpha, +4 short mode, +6 short arc, +8 float
//! height, +0xc int counter, +0x10 short start angle, +0x14 floats heights[21], +0x7e short tilt (or phase), +0x80
//! float radius.

use super::{Emitted, Owner, Prim, cos_at, emit_quad, emit_ring_quad, emit_strip, rad, sin_at};

const RECORDS: usize = 4;

fn rec(k: usize) -> usize {
    0x3e8 + k * 0xb4
}

/// The records active (+0 byte 1), as their index and offset. No update writes a record's flag, so they are taken
/// once.
fn active(p: &Prim) -> impl Iterator<Item = (usize, usize)> + use<> {
    let on: [bool; RECORDS] = std::array::from_fn(|k| p.u8(rec(k)) == 1);
    (0..RECORDS).filter(move |&k| on[k]).map(|k| (k, rec(k)))
}

/// A record's step counter +0xc, one up.
fn tick(p: &mut Prim, r: usize) -> i32 {
    let counter = p.i(r + 0xc).wrapping_add(1);
    p.set_i(r + 0xc, counter);
    counter
}

/// The update of the primitive's type (the second switch of 0x68e550). The ring types follow their owner: 0x1e,
/// 0x90, 0x95 and 0xa1 take its place first (0x68e5ce's list) and all of them again in their own update (0x1f only
/// there; 0x58 only there); taken once here, as taking it again changes nothing. 0xf and 0x10 place themselves
/// from it.
pub(super) fn update(p: &mut Prim, owner: &Owner) {
    let motion: fn(&mut Prim) = match p.kind() {
        0x0f | 0x10 => return fly(p, owner),
        0x1e => |p| grow_ring(p, true),
        0x1f => wave_ring_1f,
        0x58 => spread_squares_58,
        0x90 => |p| grow_ring(p, false),
        0x92 => drift_ring_92,
        0x95 => turn_ring_95,
        0x9f => homing_bands_9f,
        0xa1 => square_ring_a1,
        // prim_recipe_table.py's `PORTED` kinds are these: the tests' traces of every recipe would stop here
        kind => {
            debug_assert!(false, "primitive type {kind:#x} not ported");
            return;
        }
    };
    p.copy_owner_pos(owner);
    motion(p);
}

pub(super) fn render(p: &Prim, e: &mut Emitted) {
    match p.kind() {
        0x0f => upright_quad(p, e),
        0x10 => crossed_quads(p, e),
        0x1e | 0x1f => ring_wall(p, e, false),
        0x90 => ring_wall(p, e, true),
        0x92 => drifting_wall(p, e),
        0x95 => turning_wall(p, e),
        0x9f => crossed_bands(p, e),
        0x58 | 0xa1 => square_ring(p, e),
        kind => debug_assert!(false, "primitive type {kind:#x} not drawn"),
    }
}

/// The heights of a record's wall: h[j] = H + sin(9j)·H·0.3·sin(phase) (0x6c7130, 0x6c6d40, 0x69d740 alike), the
/// phase the counter plus 90 a record for records 0 and 1, twice (counter + 45 a record) for 2 and 3.
fn wobble_heights(p: &mut Prim, k: usize) {
    let r = rec(k);
    let counter = p.i(r + 0xc);
    let phase = if k < 2 {
        counter.wrapping_add(90 * k as i32)
    } else {
        counter.wrapping_add(45 * k as i32).wrapping_mul(2)
    };
    let wave = sin_at(phase % 360);
    for j in 0..21 {
        let h = p.f(r + 8);
        p.set_f(r + 0x14 + 4 * j, sin_at(9 * j as i32) * h * 0.3 * wave + h);
    }
}

/// 0x6c7130 (type 0x90) and 0x69d740 (type 0x1e, `quick_mode`): the wall leans out as it grows; once past radius 4
/// it fades and starts again from 3.37. 0x69d740's records of mode 0x16 fade in by 5 (else 10) and start again at a
/// tilt of 64 (else 74).
fn grow_ring(p: &mut Prim, quick_mode: bool) {
    let life = p.i(0x1e4);
    for (k, r) in active(p) {
        let counter = tick(p, r);
        if counter <= 0 {
            continue;
        }
        let grown = p.f(r + 0x80) + 0.07;
        p.set_f(r + 0x80, grown);
        let tilt = p.s(r + 0x7e).wrapping_sub(1);
        p.set_s(r + 0x7e, tilt);
        if tilt < 10 {
            p.set_s(r + 0x7e, 10);
            p.set_s(r + 2, 0);
        }
        let quick = quick_mode && p.s(r + 4) == 0x16;
        let alpha = p.s(r + 2);
        if grown < 4.0 {
            p.set_s(r + 2, alpha.wrapping_add(if quick { 5 } else { 10 }));
        } else if alpha.wrapping_sub(3) < 1 {
            p.set_s(r + 2, 0);
            if counter < life.wrapping_sub(30) {
                p.set_f(r + 0x80, f32::from_bits(0x4057_ae14));
                p.set_s(r + 0x7e, if quick { 64 } else { 74 });
            }
        } else {
            p.set_s(r + 2, alpha.wrapping_sub(3));
        }
        wobble_heights(p, k);
    }
}

/// 0x6c6d40 (type 0x95): the wall turns 3° a step, fades in over 30 steps (to 100) and out over its last 46.
fn turn_ring_95(p: &mut Prim) {
    let life = p.i(0x1e4);
    for (k, r) in active(p) {
        let counter = tick(p, r);
        if counter <= 0 {
            continue;
        }
        let start = p.s(r + 0x10);
        let turned = start.wrapping_add(3);
        p.set_s(
            r + 0x10,
            if turned > 359 {
                start.wrapping_sub(357)
            } else {
                turned
            },
        );
        if counter < 30 {
            p.set_s(r + 2, p.s(r + 2).wrapping_add(3).min(100));
        }
        if life.wrapping_sub(46) < counter {
            p.set_s(r + 2, p.s(r + 2).wrapping_sub(2).max(0));
        }
        wobble_heights(p, k);
    }
}

/// 0x6c6c80 (type 0xa1): records 0 and 1 (no active check) turn their phase 3° a step and fade by 8 over their
/// last 25 steps.
fn square_ring_a1(p: &mut Prim) {
    let life = p.i(0x1e4);
    for k in 0..2 {
        let r = rec(k);
        let phase = p.s(r + 0x7e).wrapping_add(3);
        let counter = tick(p, r);
        p.set_s(
            r + 0x7e,
            if phase > 359 {
                phase.wrapping_sub(360)
            } else {
                phase
            },
        );
        if life.wrapping_sub(25) < counter {
            p.set_s(r + 2, p.s(r + 2).wrapping_sub(8).max(0));
        }
    }
}

/// 0x690509 (type 0x58, inline in 0x68e550): each record with a positive flag (a signed byte) fades in over 10
/// steps (by 6 at preset +0x318 113, else 2) and out by 1 after, its radius growing 7% a step.
fn spread_squares_58(p: &mut Prim) {
    let quick = p.f(0x318) == 113.0;
    for r in (0..RECORDS).map(rec) {
        if p.u8(r) as i8 <= 0 {
            continue;
        }
        let counter = tick(p, r);
        if counter <= 0 {
            continue;
        }
        if counter <= 10 {
            p.set_s(r + 2, p.s(r + 2).wrapping_add(if quick { 6 } else { 2 }));
        }
        p.set_f(r + 0x80, p.f(r + 0x80) * f32::from_bits(0x3f88_f5c3));
        if counter > 10 {
            p.set_s(r + 2, p.s(r + 2).wrapping_sub(1).max(0));
        }
    }
}

/// An angle a turn on, past 359 (as a short) back by 360.
fn wrap_up(a: i16) -> i16 {
    if a >= 360 { a.wrapping_sub(360) } else { a }
}

/// 0x6c9500 (type 0x9f): each active record (yaw +0x10, e +0x7e, length +0x80, place P +0x84, speed +0x18) fades
/// in by 5 over 19 steps and flies P back along (yaw, e). State +0x68 0 until |P| < 12; 1: e turns 9° a step,
/// fading by 2 and shrinking 1% for 10 steps (+0x69) before state 2: e back 3°, yaw on 1°, past step 300 fading by
/// 1 and shrinking 1%.
fn homing_bands_9f(p: &mut Prim) {
    let shrink = f32::from_bits(0x3f7d_70a4);
    for r in (0..RECORDS).map(rec) {
        if p.u8(r) != 1 {
            continue;
        }
        let counter = tick(p, r);
        if counter <= 0 {
            continue;
        }
        if counter < 20 {
            p.set_s(r + 2, p.s(r + 2).wrapping_add(5).min(0xff));
        }
        let [x, y, z] = [p.f(r + 0x84), p.f(r + 0x88), p.f(r + 0x8c)];
        let far = (x * x + y * y + z * z).sqrt();
        let turning = match p.u8(r + 0x68) {
            0 if far < 12.0 => {
                // a short write: state 1, its step count +0x69 0
                p.set_s(r + 0x68, 1);
                true
            }
            1 => true,
            _ => false,
        };
        if turning {
            p.set_s(r + 0x7e, wrap_up(p.s(r + 0x7e).wrapping_add(9)));
            p.set_s(r + 0x10, wrap_up(p.s(r + 0x10)));
            let n = (p.u8(r + 0x69) as i8).wrapping_add(1);
            p.set_u8(r + 0x69, n as u8);
            if n >= 10 {
                p.set_u8(r + 0x68, 2);
            }
            fade_shrink(p, r, 2, shrink);
        }
        if p.u8(r + 0x68) == 2 {
            let e = p.s(r + 0x7e).wrapping_sub(3);
            p.set_s(
                r + 0x7e,
                wrap_up(if e < 0 { e.wrapping_add(360) } else { e }),
            );
            let yaw = p.s(r + 0x10).wrapping_add(1);
            p.set_s(
                r + 0x10,
                wrap_up(if yaw < 0 { yaw.wrapping_add(360) } else { yaw }),
            );
            if counter > 300 {
                fade_shrink(p, r, 1, shrink);
            }
        }
        let (e, yaw) = (i32::from(p.s(r + 0x7e)), i32::from(p.s(r + 0x10)));
        let speed = p.f(r + 0x18);
        let ahead = cos_at(e) * speed;
        p.set_f(r + 0x84, p.f(r + 0x84) - cos_at(yaw) * ahead);
        p.set_f(r + 0x88, p.f(r + 0x88) - (sin_at(yaw) * ahead + 0.0));
        p.set_f(r + 0x8c, p.f(r + 0x8c) - sin_at(e) * speed);
    }
}

/// A 0x9f record's alpha down by `by` (to 0 once below) and its length +0x80 times `shrink`.
fn fade_shrink(p: &mut Prim, r: usize, by: i16, shrink: f32) {
    let alpha = p.s(r + 2).wrapping_sub(by);
    p.set_s(r + 2, if alpha < 0 { 0 } else { alpha });
    p.set_f(r + 0x80, p.f(r + 0x80) * shrink);
}

type Rgb = (u32, u32, u32);

/// 0x6d2c20's thin and wide band colours by the record's colour +0x6a (other values: none drawn)
const BANDS: [(Rgb, Rgb); 4] = [
    ((0x00, 0xfa, 0xfa), (0xfa, 0x00, 0x00)),
    ((0xfa, 0xfa, 0xfa), (0xfa, 0xfa, 0x00)),
    ((0xfa, 0x64, 0x64), (0xff, 0xa6, 0xd2)),
    ((0xc8, 0xc8, 0xff), (0x05, 0x05, 0xff)),
];

/// 0x6d2c20 (type 0x9f): each active record with alpha two thin quads (0x6c3a10) across a circle of radius
/// +0x80 about P +0x84 in the plane of its yaw +0x10, Q(φ) = P + (cos yaw cos φ L, sin yaw cos φ L + +0x14,
/// sin φ L): Q(e−2), Q(e+2), Q(e+178), Q(e+182), then ±8 and 172, 188; coloured by +0x6a (other values: none).
fn crossed_bands(p: &Prim, e: &mut Emitted) {
    for r in (0..RECORDS).map(rec) {
        let alpha = p.s(r + 2);
        if p.u8(r) != 1 || alpha <= 0 {
            continue;
        }
        let at = i32::from(p.s(r + 0x7e));
        let yaw = i32::from(p.s(r + 0x10));
        let (cy, sy) = (cos_at(yaw), sin_at(yaw));
        let (l, lift) = (p.f(r + 0x80), p.f(r + 0x14));
        let [px, py, pz] = [p.f(r + 0x84), p.f(r + 0x88), p.f(r + 0x8c)];
        let q = |phi: i32| {
            [
                cy * cos_at(phi) * l + px,
                sy * cos_at(phi) * l + lift + py,
                l * sin_at(phi) + pz,
            ]
        };
        // the four angles kept within 0 to 359 as the exe picks each offset (one wrap: never past ±360)
        let around = |d: i32| (at + d).rem_euclid(360);
        let Some(&(thin, wide)) = BANDS.get(usize::from(p.u8(r + 0x6a))) else {
            continue;
        };
        for (rgb, d) in [(thin, 2), (wide, 8)] {
            let corners = [
                q(around(-d)),
                q(around(d)),
                q(around(180 - d)),
                q(around(180 + d)),
            ];
            emit_quad(p, e, alpha, corners, 0, 0, rgb, (0.0, 1.0, 0.0, 1.0), 0);
        }
    }
}

/// 0x69d8e0 (type 0x1f): the wall turns and widens (3° and 0.1 a step when +0x2c0 is 99, else 5° and 0.3), leans
/// out to a tilt of 30 (40), fades in over 10 steps and out by its mode; heights H·sin(9j)·(1 + 0.15·sin(phase)).
fn wave_ring_1f(p: &mut Prim) {
    let slow = p.f(0x2c0) == 99.0;
    for (k, r) in active(p) {
        // this type keeps its counter as a float (69d8e0: read truncated, one added, stored back as a float)
        let counter = (p.f(r + 0xc) as i32).wrapping_add(1);
        p.set_f(r + 0xc, counter as f32);
        let angle = p.s(r + 0x10).wrapping_add(if slow { 3 } else { 5 });
        p.set_s(
            r + 0x10,
            if angle > 359 {
                angle.wrapping_sub(360)
            } else {
                angle
            },
        );
        let floor = if slow { 30 } else { 40 };
        p.set_s(r + 0x7e, p.s(r + 0x7e).wrapping_sub(1).max(floor));
        p.set_f(
            r + 0x80,
            p.f(r + 0x80)
                + if slow {
                    f32::from_bits(0x3dcc_cccd)
                } else {
                    0.3
                },
        );
        let mode = p.s(r + 4);
        if counter < 11 {
            p.set_s(
                r + 2,
                p.s(r + 2).wrapping_add(if mode == 1 { 10 } else { 20 }),
            );
        } else {
            let step = match mode {
                2 => 8,
                3 | 4 => 5,
                5 => 4,
                _ => 2,
            };
            let alpha = p.s(r + 2).wrapping_sub(step);
            p.set_s(r + 2, if alpha < 1 { 0 } else { alpha });
        }
        let wave = sin_at(counter.wrapping_add(90 * k as i32) % 360);
        let h = p.f(r + 8);
        for j in 0..21 {
            let s = sin_at(9 * j as i32);
            p.set_f(r + 0x14 + 4 * j, h * s * 0.15 * wave + h * s);
        }
    }
}

/// 0x698930's mark of a second-set value not given (0.0001, 0xd2a630).
const UNSET: f32 = f32::from_bits(0x38d1_b717);
/// 0x698930's flags the port does not replay (prim_recipe_table.py leaves their recipes out): 4 the ground, 0x10 a
/// write into the owner, 0x40 and 0x200 the camera, 0x400 the owner's target.
const FLY_UNREPLAYED: u32 = 0x4 | 0x10 | 0x40 | 0x200 | 0x400;

/// 0x698930 (types 0xf and 0x10): placed at the owner's place + +0x394 + the offset +0x388, which moves each step by
/// +0x2bc along the third row of matrix +0x160 (flag 0x100: by the vector +0x370, itself growing by +0x3ac); the
/// sizes +0x354 and +0x360 grow by +0x358 and +0x364 (+0x360 stops past +0x36c), the angles +0x30c and +0x310 by
/// +0x330 and +0x334, matrix +0x190 built from them; at count +0x200 a second set of rates takes over. Then its
/// colour cycle, its alpha (0x6a5d10) and every +0x1e8 steps the next texture frame.
fn fly(p: &mut Prim, owner: &Owner) {
    let flags = p.i(0x1dc) as u32;
    debug_assert_eq!(flags & FLY_UNREPLAYED, 0, "flags {flags:#x} not replayed");
    for i in 0..3 {
        p.set_f(0x37c + 4 * i, owner.pos[i] + p.f(0x394 + 4 * i));
    }
    if p.i(0x200) == p.i(0x1e0) {
        p.set_i(0x138, 1);
        for (from, to) in [
            (0x2c8, 0x358),
            (0x2cc, 0x364),
            (0x29c, 0x2bc),
            (0x2c4, 0x35c),
            (0x2d0, 0x368),
            (0x2a0, 0x2c0),
            (0x2a4, 0x370),
            (0x2a8, 0x374),
            (0x2ac, 0x378),
        ] {
            let v = p.f(from);
            if !v.is_nan() && v != UNSET {
                p.set_f(to, v);
            }
        }
        for off in [
            0x2c8, 0x2cc, 0x29c, 0x2c4, 0x2d0, 0x2a0, 0x2a4, 0x2a8, 0x2ac, 0x2b0, 0x2b4, 0x2b8,
        ] {
            p.set_f(off, UNSET);
        }
    }
    for i in 0..3 {
        p.set_f(0x370 + 4 * i, p.f(0x370 + 4 * i) + p.f(0x3ac + 4 * i));
    }
    let speed = p.f(0x2c0) + p.f(0x2bc);
    p.set_f(0x2bc, speed);
    for (v, rate) in [
        (0x358, 0x35c),
        (0x364, 0x368),
        (0x330, 0x33c),
        (0x334, 0x340),
    ] {
        p.set_f(v, p.f(v) + p.f(rate));
    }
    for i in 0..3 {
        if flags & 0x100 == 0 {
            // (0, 0, speed) through matrix +0x160: its third row and translation
            p.set_f(
                0x370 + 4 * i,
                p.f(0x178 + 4 * i) * speed + p.f(0x184 + 4 * i),
            );
        }
        p.set_f(0x388 + 4 * i, p.f(0x388 + 4 * i) + p.f(0x370 + 4 * i));
        p.set_f(0x10 + 4 * i, p.f(0x37c + 4 * i) + p.f(0x388 + 4 * i));
    }
    let height = p.f(0x364) + p.f(0x360);
    for (v, rate) in [(0x354, 0x358), (0x30c, 0x330), (0x310, 0x334)] {
        p.set_f(v, p.f(v) + p.f(rate));
    }
    p.set_f(0x360, height);
    if height > p.f(0x36c) {
        p.set_f(0x368, 0.0);
        p.set_f(0x364, 0.0);
    }
    // 0x4de890, 0x4dc380, 0x4dc620 on +0x190 (0x698ef4): rotZ(+0x314) · rotX(+0x310) · rotY(+0x30c)
    let turn = |off| {
        let a = f64::from(rad(p.f(off) as i32));
        (a.sin() as f32, a.cos() as f32)
    };
    let (sz, cz) = turn(0x314);
    let (sx, cx) = turn(0x310);
    let (sy, cy) = turn(0x30c);
    let mut m = [cz, sz, 0.0, -sz, cz, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    for row in m.chunks_exact_mut(3) {
        let (y, z) = (row[1], row[2]);
        row[1] = y * cx + -sx * z;
        row[2] = y * sx + z * cx;
    }
    for row in m.chunks_exact_mut(3) {
        let (x, z) = (row[0], row[2]);
        row[0] = x * cy + z * sy;
        row[2] = -sy * x + z * cy;
    }
    for (i, v) in m.into_iter().enumerate() {
        p.set_f(0x190 + 4 * i, v);
    }
    if flags & 2 != 0 {
        let at = p.i(0x1d4);
        p.set_i(0x1d4, if at + 1 == 10 { 0 } else { at + 1 });
        let v = at as f32 / 10.0;
        p.set_f(0x2e4, v);
        p.set_f(0x2e8, v + 1.0);
    }
    let count = p.i(0x1e0);
    if flags & 0x20 != 0 && count % divisor(p, 0x1cc) == 0 {
        p.set_f(0x364, -p.f(0x364));
    }
    colour_cycle(p, flags & 0x800 != 0);
    fade(p, owner);
    if count % divisor(p, 0x1e8) == 0 {
        let frame = p.i(0x3c).wrapping_add(1);
        // with +0x254 clear it stops at +0x204 (its answer ignored by 0x68ea09)
        p.set_i(
            0x3c,
            if p.u8(0x254) == 0 {
                frame
            } else {
                frame % frames(p)
            },
        );
    }
}

/// 0x698930's colour cycle: red, green and blue +0x23c, +0x240, +0x244 step by +0x248, +0x24c, +0x250; red wraps
/// (past 255 to 0, below 0 to 255), green and blue clamp to 0 to 255. `slow` (flag 0x800): while a channel is 254
/// the step waits +0x234 updates (counter +0x1fc), and every channel wraps past 254 to 0, below 0 to 254.
fn colour_cycle(p: &mut Prim, slow: bool) {
    let rgb = [0x23c, 0x240, 0x244];
    let steps = if !slow || rgb.iter().all(|&o| p.i(o) != 0xfe) {
        true
    } else {
        let mut waited = p.i(0x1fc);
        if p.i(0x234) != 0 {
            waited += 1;
            p.set_i(0x1fc, waited);
        }
        let due = p.i(0x234) == waited;
        if due {
            p.set_i(0x1fc, 0);
        }
        due
    };
    if steps {
        for (c, by) in rgb.into_iter().zip([0x248, 0x24c, 0x250]) {
            p.set_i(c, p.i(c) + p.i(by));
        }
    }
    for (k, c) in rgb.into_iter().enumerate() {
        let v = p.i(c);
        let v = match (slow, k) {
            (true, _) if v >= 0xff => 0,
            (true, _) if v < 0 => 0xfe,
            (false, 0) if v >= 0x100 => 0,
            (false, 0) if v < 0 => 0xff,
            (false, _) => v.clamp(0, 0xff),
            _ => v,
        };
        p.set_i(c, v);
    }
}

/// 0x6a5d10 without key tables (+0x150, +0x154 clear): the alpha +0x324 steps by +0x328 within +0x2d8 to +0x2d4;
/// flag 0x80 turns the step back at either end. Outliving its owner by more than one update, it fades out over the
/// owner's remaining updates (once, +0x15c); at count +0x1f0 it fades out over its own (by +0x32c). Colour +0x1c0 =
/// the alpha and the low bytes of +0x23c, +0x240, +0x244.
fn fade(p: &mut Prim, owner: &Owner) {
    debug_assert!(
        p.i(0x150) == 0 && p.i(0x154) == 0,
        "alpha key tables not replayed"
    );
    let flags = p.i(0x1dc) as u32;
    let alpha = p.f(0x324);
    if flags & 0x80 != 0 {
        let inside = if p.f(0x328) >= 0.0 {
            alpha < p.f(0x2d4)
        } else {
            p.f(0x2d8) < alpha
        };
        if !inside {
            p.set_f(0x328, -p.f(0x328));
        }
    }
    let (count, life) = (p.i(0x1e0), p.i(0x1e4));
    let left = owner.length - owner.count;
    // an owner whose length is not known (-1) never cuts its primitives short
    if p.i(0x15c) == 0 && owner.length >= 0 && left + 1 < life - count {
        p.set_i(0x15c, 1);
        p.set_i(0x1dc, (flags & !0x80) as i32);
        p.set_f(0x328, -(alpha / left as f32));
    }
    if count == p.i(0x1f0) {
        p.set_i(0x1dc, (flags & !0x80) as i32);
        p.set_f(0x328, -(alpha / (life - count) as f32 * p.f(0x32c)));
    }
    let stepped = alpha + p.f(0x328);
    let floor = p.f(0x2d8);
    let lifted = if floor <= stepped { stepped } else { floor };
    let alpha = if lifted <= p.f(0x2d4) {
        lifted
    } else {
        p.f(0x2d4)
    };
    p.set_f(0x324, alpha);
    let byte = |o| u32::from(p.u8(o));
    let argb = (((alpha as i32 as u32).wrapping_mul(0x100) + byte(0x23c)) * 0x100 + byte(0x240))
        * 0x100
        + byte(0x244);
    p.set_i(0x1c0, argb as i32);
}

/// `pt` through the 3×4 matrix at `m` (rows x, y, z, then the translation), in the exe's order of sums.
fn through(p: &Prim, m: usize, pt: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| {
        p.f(m + 4 * i) * pt[0]
            + p.f(m + 0xc + 4 * i) * pt[1]
            + p.f(m + 0x18 + 4 * i) * pt[2]
            + p.f(m + 0x24 + 4 * i)
    })
}

/// 0x6ad4b0 (type 0xf): one upright quad (0, y, ±W) through matrix +0x190, W +0x354, from y −H to H (H +0x360; flag
/// 0x80: −2H to 0; flag 8: bottom 0), v +0x2e4 at the top, +0x2e8 at the bottom; filed with +0x1c4.
fn upright_quad(p: &Prim, e: &mut Emitted) {
    let flags = p.i(0x1dc) as u32;
    let (w, h) = (p.f(0x354), p.f(0x360));
    let (top, bottom) = if flags & 0x80 != 0 {
        (h * -2.0, 0.0)
    } else {
        (-h, h)
    };
    let bottom = if flags & 8 != 0 { 0.0 } else { bottom };
    let pts = [
        [0.0, top, w],
        [0.0, top, -w],
        [0.0, bottom, w],
        [0.0, bottom, -w],
    ]
    .map(|q| through(p, 0x190, q));
    emit_strip(p, e, pts, (p.f(0x2e4), p.f(0x2e8)));
}

/// 0x6a9ff0 (type 0x10): two crossed quads through matrix +0x160, H +0x360, W +0x354: (∓H, 0, ±W) and (0, ∓H, ±W),
/// v from 1 to 0; filed with flag 1.
fn crossed_quads(p: &Prim, e: &mut Emitted) {
    let (w, h) = (p.f(0x354), p.f(0x360));
    for quad in [
        [[-h, 0.0, w], [-h, 0.0, -w], [h, 0.0, w], [h, 0.0, -w]],
        [[0.0, -h, w], [0.0, -h, -w], [0.0, h, w], [0.0, h, -w]],
    ] {
        emit_strip(p, e, quad.map(|q| through(p, 0x160, q)), (1.0, 0.0));
    }
}

/// The texture frames +0x204 (kRO divides by it; none would trap there).
fn frames(p: &Prim) -> i32 {
    let n = p.i(0x204);
    debug_assert!(n > 0, "a primitive with no texture frames");
    n.max(1)
}

/// A field kRO divides an update count by (0 would trap there; `prim_recipe_table.py` replays none)
fn divisor(p: &Prim, at: usize) -> i32 {
    let n = p.i(at);
    debug_assert!(n != 0, "a primitive dividing by 0 at {at:#x}");
    if n == 0 { 1 } else { n }
}

/// The base of a wall by its height (0x6a9210 and 0x6cbc10: exact compares with 6.001 and 24.001).
fn wall_lift(h: f32) -> f32 {
    if h == f32::from_bits(0x40c0_0831) {
        2.0
    } else if h == f32::from_bits(0x41c0_020c) {
        1.0
    } else {
        0.0
    }
}

/// A record's wall along its arc (0x6a9210, 0x6cbc10 and 0x6cb2a0 alike): points every span / 20 degrees from its
/// start (a whole ring's 21st back at the start), each with its height (held at the 21st past it), `point` making
/// its base and top from the angle and height; `quad` given each two points' segment and corners A, B, C, D (base
/// and top of the one before, top and base of the next).
fn wall(
    p: &Prim,
    r: usize,
    point: impl Fn(i32, f32) -> ([f32; 3], [f32; 3]),
    mut quad: impl FnMut(i32, [[f32; 3]; 4]),
) {
    let span = i32::from(p.s(r + 6));
    if span < 0 {
        return;
    }
    // kRO's loop steps span / 20, so never ends for a span of 0 to 19; no replayed recipe has one
    let stride = span / 20;
    debug_assert!(stride > 0, "a wall of span {span} never ends in kRO");
    if stride == 0 {
        return;
    }
    let start = i32::from(p.s(r + 0x10));
    let (mut j, mut off) = (0usize, 0);
    let mut prev: Option<([f32; 3], [f32; 3])> = None;
    while off <= span {
        let a = if span == 360 && j == 20 {
            start
        } else {
            (off + start) % 360
        };
        let (base, top) = point(a, p.f(r + 0x14 + 4 * j));
        if let Some((prev_base, prev_top)) = prev {
            quad(j as i32 - 1, [prev_base, base, top, prev_top]);
        }
        prev = Some((base, top));
        j = (j + 1).min(20);
        off += stride;
    }
}

/// 0x6a9210 (types 0x1e, 0x1f; with `animated`, 0x6cbc10 for 0x90): each active record a wall of up to 20 quads
/// along its arc, from the radius out by the tilted heights; 0x90's texture is its counter's frame (% +0x204).
fn ring_wall(p: &Prim, e: &mut Emitted, animated: bool) {
    let preset = p.f(0x318) as i32;
    for (_, r) in active(p) {
        let tilt = i32::from(p.s(r + 0x7e));
        let (lean, rise) = (cos_at(tilt), sin_at(tilt));
        let radius = p.f(r + 0x80);
        let lift = wall_lift(p.f(r + 8));
        let alpha = p.s(r + 2);
        let tex = if animated {
            p.i(r + 0xc) % frames(p)
        } else {
            0
        };
        wall(
            p,
            r,
            |a, h| {
                let base = [radius * cos_at(a), -lift, radius * sin_at(a)];
                let out = lean * h;
                let top = [
                    out * cos_at(a) + base[0],
                    -(rise * h + lift),
                    out * sin_at(a) + base[2],
                ];
                (base, top)
            },
            |seg, corners| emit_ring_quad(p, e, preset, seg, alpha, corners, tex),
        );
    }
}

/// 0x6c9160 (type 0x92): each active record drifts one unit a step along its start angle − 15° (+0x90, +0x98),
/// fades in by 25 over 9 steps (to 250) and out by 10; once faded out it starts again leaning 20° less and 0.4
/// narrower, until its tilt is below 41. Heights h[j] = H·(0.4, 0.5, …, 0.9, then 1.0, mirrored), times
/// sin(6·counter) while the counter is below 16.
fn drift_ring_92(p: &mut Prim) {
    const SHAPE: [u32; 6] = [
        0x3ecc_cccd,
        0x3f00_0000,
        0x3f19_999a,
        0x3f33_3334,
        0x3f4c_cccd,
        0x3f66_6666,
    ];
    for r in (0..RECORDS).map(rec) {
        if p.u8(r) != 1 {
            continue;
        }
        let counter = tick(p, r);
        if counter <= 0 {
            continue;
        }
        let start = p.s(r + 0x10);
        let back = start.wrapping_add(345);
        let a = i32::from(if back < 360 {
            back
        } else {
            start.wrapping_sub(15)
        });
        p.set_f(r + 0x90, cos_at(a) + p.f(r + 0x90));
        p.set_f(r + 0x98, sin_at(a) + p.f(r + 0x98));
        p.set_s(r + 0x10, wrap_up(start));
        if counter < 10 {
            // 0x01138cbc (not read; 0 at start-up) set would cap it at 125
            p.set_s(r + 2, p.s(r + 2).wrapping_add(25).min(250));
        } else {
            let alpha = p.s(r + 2).wrapping_sub(10);
            p.set_s(r + 2, alpha);
            if alpha < 0 {
                let tilt = p.s(r + 0x7e).wrapping_sub(20);
                p.set_s(r + 0x7e, tilt);
                p.set_s(r + 2, 0);
                p.set_i(r + 0xc, 0);
                p.set_f(r + 0x80, p.f(r + 0x80) - f32::from_bits(SHAPE[0]));
                if tilt <= 40 {
                    p.set_u8(r, 0);
                    p.set_s(r + 0x7e, 40);
                }
            }
        }
        let counter = p.i(r + 0xc);
        for j in 0..21 {
            let shape = SHAPE.get(j.min(20 - j)).map_or(1.0, |&b| f32::from_bits(b));
            let mut h = p.f(r + 8) * shape;
            if counter < 16 {
                h *= sin_at(counter * 6);
            }
            p.set_f(r + 0x14 + 4 * j, if 0.0 > h { 0.0 } else { h });
        }
    }
}

/// 0x6d1ea0 (type 0x92): each active record's wall along its arc (as 0x6a9210), the radius +0x80 about the place
/// record 0 drifted to (P = +0x478, +0x480): its base at −0.75·P, its top at −P; coloured by the record's preset
/// +0xa8, its texture the counter's frame.
fn drifting_wall(p: &Prim, e: &mut Emitted) {
    let (px, pz) = (p.f(0x478), p.f(0x480));
    let three_quarters = f32::from_bits(0x3f40_0000);
    for (_, r) in active(p) {
        let counter = p.i(r + 0xc);
        if counter <= 0 {
            continue;
        }
        let tilt = i32::from(p.s(r + 0x7e));
        let radius = p.f(r + 0x80);
        let (alpha, preset) = (p.s(r + 2), p.f(r + 0xa8) as i32);
        let tex = counter % frames(p);
        wall(
            p,
            r,
            |a, h| {
                let (x, z) = (radius * cos_at(a), radius * sin_at(a));
                let out = cos_at(tilt) * h;
                let base = [x - px * three_quarters, 0.0, z - pz * three_quarters];
                let top = [
                    out * cos_at(a) + x - px,
                    -(sin_at(tilt) * h),
                    out * sin_at(a) + z - pz,
                ];
                (base, top)
            },
            |seg, corners| emit_ring_quad(p, e, preset, seg, alpha, corners, tex),
        );
    }
}

/// 0x6cb2a0 (type 0x95): each active record's wall, its radius swaying by sin(start): drawn white (preset 0) at its
/// alpha, then again in its mode's colour at 2 or 3 times it (mode 10: only that, at a third of the frame rate).
fn turning_wall(p: &Prim, e: &mut Emitted) {
    for (_, r) in active(p) {
        let tilt = i32::from(p.s(r + 0x7e));
        let (lean, rise) = (cos_at(tilt), sin_at(tilt));
        let sway = sin_at(i32::from(p.s(r + 0x10))) + p.f(r + 0x80);
        let (mode, alpha, counter) = (p.s(r + 4), p.s(r + 2), p.i(r + 0xc));
        let tint = match mode {
            m if m < 3 => Some((17, 3)),
            3 => Some((18, 3)),
            4 => Some((19, 2)),
            5 => Some((20, 2)),
            6 => Some((5, 2)),
            7 => Some((21, 2)),
            8 => Some((22, 2)),
            // 0x01138cbc (not read; 0 at start-up) set would draw these at their alpha
            9 => Some((30, 2)),
            10 => Some((6, 3)),
            11 => Some((32, 2)),
            // other modes keep the alpha of a previous pass (not replayed)
            _ => None,
        };
        wall(
            p,
            r,
            |a, h| {
                let base = [sway * cos_at(a), 0.0, sway * sin_at(a)];
                let out = lean * h;
                let top = [
                    out * cos_at(a) + base[0],
                    -(rise * h),
                    out * sin_at(a) + base[2],
                ];
                (base, top)
            },
            |seg, corners| {
                let tex = if mode == 10 {
                    (counter / 3) % frames(p)
                } else {
                    let tex = counter % frames(p);
                    emit_ring_quad(p, e, 0, seg, alpha, corners, tex);
                    tex
                };
                if let Some((preset, times)) = tint {
                    let scaled = alpha.wrapping_mul(times).min(255);
                    emit_ring_quad(p, e, preset, seg, scaled, corners, tex);
                }
            },
        );
    }
}

/// 0x6a6ee0 (types 0x58 and 0xa1): each active record one flat square at its height, its corners 90° apart from θ (the
/// record's start for presets 0, 10, 100, 110 and up, or 4 with +0x24 at 4; else 23° a record), at a radius
/// pulsing between 0.8 and 1.0 of its own; coloured by the preset +0x318 (0x6c3a10).
fn square_ring(p: &Prim, e: &mut Emitted) {
    let m = p.f(0x318);
    for (k, r) in active(p) {
        let own =
            m == 100.0 || m >= 110.0 || (m == 4.0 && p.f(r + 0x24) == 4.0) || m == 10.0 || m == 0.0;
        let theta = if own {
            i32::from(p.s(r + 0x10))
        } else {
            23 * k as i32
        };
        let radius = p.f(r + 0x80);
        let pulse = f64::from(
            (sin_at(i32::from(p.s(r + 0x7e))) + 1.0) * radius * f32::from_bits(0x3dcc_cccd),
        ) + f64::from(radius) * 0.8;
        let y = p.f(r + 0x14);
        let corner = |a: i32| {
            [
                (f64::from(cos_at(a)) * pulse) as f32,
                y,
                (f64::from(sin_at(a)) * pulse) as f32,
            ]
        };
        let quarter = |a: i32| if a + 90 < 360 { a + 90 } else { a - 270 };
        let a1 = quarter(theta);
        let a2 = if a1 + 90 > 359 { a1 - 270 } else { a1 + 90 };
        let a3 = quarter(a2);
        let corners = [corner(theta), corner(a1), corner(a2), corner(a3)];
        let alpha = p.s(r + 2);
        let uv = (0.0, 1.0, 0.0, 1.0);
        let white = (0xff, 0xff, 0xff);
        // 0x6a6ee0's compares of +0x318, in its order
        let (flag, rgb, mode) = if m == 101.0 {
            emit_quad(p, e, alpha, corners, 0, 0, (0xff, 0, 0), uv, 0);
            (0, (0xff, 0, 0), 0)
        } else if m == 102.0 {
            (0, (0xff, 0xff, 0), 0)
        } else if m == 111.0 {
            (0, (0x37, 0x37, 0xff), 0)
        } else if m == 112.0 {
            (0, (0x69, 0xe1, 0x69), 0)
        } else if m == 100.0 {
            emit_quad(p, e, alpha, corners, 0, 0, white, uv, 0);
            (0, white, 0)
        } else if m == 110.0 {
            (1, white, 0)
        } else if m == 113.0 {
            (0, (5, 5, 0xff), 1)
        } else if m == 114.0 {
            (0, (0x9b, 5, 5), 1)
        } else if m == 15.0 {
            (0, (0x23, 0xff, 0x23), 0)
        } else if m == 17.0 {
            (0, (0x69, 0x69, 0xff), 0)
        } else if m == 18.0 {
            (0, (0x23, 0x23, 0xff), 0)
        } else if m == 0.0 {
            (1, white, 0)
        } else {
            (0, white, 0)
        };
        emit_quad(p, e, alpha, corners, flag, 0, rgb, uv, mode);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{cos, sin};

    #[test]
    fn angles_wrap_as_the_exe_does() {
        assert_eq!(cos(0), 1.0);
        assert_eq!(sin(90), 1.0);
        // a negative angle reads the table from its end: -1 is cos 360
        assert_eq!(cos(-1), cos(360));
        assert_eq!(sin(-30), sin(331));
        // past ±360 the C remainder first
        assert_eq!(cos(450), cos(90));
        assert_eq!(sin(-450), sin(-90));
    }
}
