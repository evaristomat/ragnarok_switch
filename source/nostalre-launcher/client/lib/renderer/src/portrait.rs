//! A picture of an actor composed on the CPU from the same layers the game draws: the clips
//! [`build_composite_clips`] places, in the order [`CompositeClips::into_draw_order`] gives, one sprite pixel to one
//! picture pixel. The Android hub shows it for each character.

use nostalre_formats::act::{ActFile, SpriteAnimationState};
use nostalre_formats::spr::{RgbaImageData, SpriteData};

use crate::sprite::{
    ClipQuad, CompositeClips, EntitySprite, SpriteSheet, build_composite_clips, scale_clip_vertices,
};

/// The picture's size in pixels.
pub const PORTRAIT_SIZE: (u32, u32) = (200, 220);
/// Where the actor's feet (the cell centre its sprite stands on) land in the picture.
pub const PORTRAIT_FEET: [f32; 2] = [100.0, 190.0];

/// A sprite's images kept as pixels.
pub struct CpuSheet {
    pub images: Vec<RgbaImageData>,
    pub indexed_count: usize,
}

impl CpuSheet {
    /// The sheet of a loaded sprite, and its act.
    pub fn from_data(data: SpriteData) -> (Self, ActFile) {
        let sheet = Self {
            images: data.images,
            indexed_count: data.indexed_count,
        };
        (sheet, data.act)
    }
}

impl SpriteSheet for CpuSheet {
    fn indexed_count(&self) -> usize {
        self.indexed_count
    }

    fn image_size(&self, index: usize) -> Option<(u32, u32)> {
        self.images.get(index).map(|img| (img.width, img.height))
    }

    fn drawable_count(&self) -> usize {
        self.images.len()
    }
}

/// Straight (not premultiplied) RGBA pixels, rows top to bottom.
pub struct Portrait {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Portrait {
    pub fn encode_png(&self) -> image::ImageResult<Vec<u8>> {
        use image::ImageEncoder;
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out).write_image(
            &self.rgba,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )?;
        Ok(out)
    }
}

/// The actor standing idle facing the camera, frame 0, with the head straight: every layer it wears, `scale` times
/// its sprite size about its feet (a baby's body is drawn smaller). No shadow.
pub fn compose_portrait(sprite: &EntitySprite<CpuSheet>, scale: f32) -> Portrait {
    let idle = SpriteAnimationState::new(0);
    let action_idx = idle.flat_action_index(&sprite.body_act);
    let clips = build_composite_clips(
        sprite,
        action_idx,
        idle.motion_index(),
        0,
        PORTRAIT_FEET,
        0.0,
    );
    let mut canvas = Canvas::new(PORTRAIT_SIZE);
    for (part, (mut vertices, tex_idx)) in clips
        .map(|clips: CompositeClips| clips.into_draw_order(action_idx, sprite.layer_order.as_ref()))
        .unwrap_or_default()
    {
        let Some(image) = sprite
            .sheet(part)
            .and_then(|sheet| sheet.images.get(tex_idx))
        else {
            continue;
        };
        scale_clip_vertices(&mut vertices, PORTRAIT_FEET, scale, [0.0, 0.0]);
        canvas.draw(&(vertices, tex_idx), image);
    }
    canvas.into_portrait()
}

/// Premultiplied RGBA in 0..1, so layers blend over a transparent background.
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    fn new((width, height): (u32, u32)) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0.0; 4]; (width * height) as usize],
        }
    }

    /// Draws a clip's quad (corners top-left, top-right, bottom-right, bottom-left before any turn) as the sprite
    /// shader does: nearest texel, times the clip colour, texels under 1% alpha dropped, blended over what is there.
    fn draw(&mut self, (vertices, _): &ClipQuad, image: &RgbaImageData) {
        if image.width == 0 || image.height == 0 {
            return;
        }
        // a quad an odd number of pixels wide is centred on a half pixel: move it onto whole ones so each picture
        // pixel samples the middle of one texel
        let shift = [
            vertices[0].position[0].round() - vertices[0].position[0],
            vertices[0].position[1].round() - vertices[0].position[1],
        ];
        let corner = |i: usize| {
            [
                vertices[i].position[0] + shift[0],
                vertices[i].position[1] + shift[1],
            ]
        };
        let (origin, right, down) = (corner(0), corner(1), corner(3));
        let e1 = [right[0] - origin[0], right[1] - origin[1]];
        let e2 = [down[0] - origin[0], down[1] - origin[1]];
        let det = e1[0] * e2[1] - e1[1] * e2[0];
        if det.abs() < 1e-6 {
            return;
        }
        let corners = [corner(0), corner(1), corner(2), corner(3)];
        let min_x = corners.iter().map(|c| c[0]).fold(f32::MAX, f32::min);
        let max_x = corners.iter().map(|c| c[0]).fold(f32::MIN, f32::max);
        let min_y = corners.iter().map(|c| c[1]).fold(f32::MAX, f32::min);
        let max_y = corners.iter().map(|c| c[1]).fold(f32::MIN, f32::max);
        let x0 = min_x.floor().max(0.0) as u32;
        let y0 = min_y.floor().max(0.0) as u32;
        let x1 = (max_x.ceil().max(0.0) as u32).min(self.width);
        let y1 = (max_y.ceil().max(0.0) as u32).min(self.height);

        let uv = |i: usize| vertices[i].tex_coord;
        let (uv0, uv1, uv3) = (uv(0), uv(1), uv(3));
        let color = vertices[0].color;
        if color[3] <= 0.0 {
            return;
        }
        const EDGE: f32 = 1e-4;
        for y in y0..y1 {
            for x in x0..x1 {
                let d = [x as f32 + 0.5 - origin[0], y as f32 + 0.5 - origin[1]];
                let s = (d[0] * e2[1] - d[1] * e2[0]) / det;
                let t = (e1[0] * d[1] - e1[1] * d[0]) / det;
                if !(-EDGE..1.0 - EDGE).contains(&s) || !(-EDGE..1.0 - EDGE).contains(&t) {
                    continue;
                }
                let u = uv0[0] + s * (uv1[0] - uv0[0]) + t * (uv3[0] - uv0[0]);
                let v = uv0[1] + s * (uv1[1] - uv0[1]) + t * (uv3[1] - uv0[1]);
                let tx = ((u * image.width as f32).floor() as i64).clamp(0, image.width as i64 - 1);
                let ty =
                    ((v * image.height as f32).floor() as i64).clamp(0, image.height as i64 - 1);
                let at = (ty as usize * image.width as usize + tx as usize) * 4;
                let Some(texel) = image.data.get(at..at + 4) else {
                    continue;
                };
                let tex_alpha = f32::from(texel[3]) / 255.0;
                if tex_alpha < 0.01 {
                    continue;
                }
                let alpha = (tex_alpha * color[3]).min(1.0);
                let dst = &mut self.pixels[(y * self.width + x) as usize];
                for c in 0..3 {
                    let src = (f32::from(texel[c]) / 255.0 * color[c]).clamp(0.0, 1.0);
                    dst[c] = src * alpha + dst[c] * (1.0 - alpha);
                }
                dst[3] = alpha + dst[3] * (1.0 - alpha);
            }
        }
    }

    fn into_portrait(self) -> Portrait {
        let mut rgba = Vec::with_capacity(self.pixels.len() * 4);
        for [r, g, b, a] in self.pixels {
            let straight = |c: f32| {
                if a > 0.0 {
                    (c / a * 255.0).round().clamp(0.0, 255.0) as u8
                } else {
                    0
                }
            };
            rgba.extend([
                straight(r),
                straight(g),
                straight(b),
                (a * 255.0).round() as u8,
            ]);
        }
        Portrait {
            width: self.width,
            height: self.height,
            rgba,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_formats::act::{Action, AnchorPoint, Motion, SpriteFrame};

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn image(width: u32, height: u32, texel: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImageData {
        let mut data = Vec::new();
        for y in 0..height {
            for x in 0..width {
                data.extend(texel(x, y));
            }
        }
        RgbaImageData {
            width,
            height,
            data,
        }
    }

    fn clip(x: i32, y: i32, mirror: bool) -> SpriteFrame {
        SpriteFrame {
            x,
            y,
            sprite_index: 0,
            mirror: u32::from(mirror),
            color: [255, 255, 255, 255],
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0,
            sprite_type: 1,
            width: None,
            height: None,
        }
    }

    /// An act whose 8 idle directions each hold one frame of `clip`, anchored at `attach`.
    fn act(clip: impl Fn() -> SpriteFrame, attach: (i32, i32)) -> ActFile {
        let motion = || Motion {
            range1: [0; 4],
            range2: [0; 4],
            clips: vec![clip()],
            event_id: -1,
            attach_points: vec![AnchorPoint {
                ignored: 0,
                x: attach.0,
                y: attach.1,
                attribute: 0,
            }],
        };
        ActFile {
            version: (2, 5),
            actions: (0..8)
                .map(|_| Action {
                    motions: vec![motion()],
                })
                .collect(),
            events: Vec::new(),
            delays: Vec::new(),
        }
    }

    fn sprite(images: RgbaImageData, act: ActFile) -> SpriteData {
        SpriteData {
            texture_key: None,
            images: vec![images],
            indexed_count: 0,
            act,
        }
    }

    fn pixel(portrait: &Portrait, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * portrait.width + x) * 4) as usize;
        portrait.rgba[at..at + 4].try_into().unwrap()
    }

    fn actor(body: SpriteData, head: Option<SpriteData>) -> EntitySprite<CpuSheet> {
        EntitySprite::from_parts(
            body,
            head,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            CpuSheet::from_data,
        )
    }

    #[test]
    fn the_body_stands_on_the_feet_point_one_texel_a_pixel() {
        // a 4x6 body whose clip centre is 3 px above the anchor: its bottom row rests on the feet
        let body = sprite(
            image(4, 6, |x, _| if x < 2 { RED } else { BLUE }),
            act(|| clip(0, -3, false), (0, 0)),
        );
        let portrait = compose_portrait(&actor(body, None), 1.0);
        assert_eq!((portrait.width, portrait.height), PORTRAIT_SIZE);
        let (fx, fy) = (PORTRAIT_FEET[0] as u32, PORTRAIT_FEET[1] as u32);
        assert_eq!(pixel(&portrait, fx - 2, fy - 1), RED);
        assert_eq!(pixel(&portrait, fx - 1, fy - 6), RED);
        assert_eq!(pixel(&portrait, fx, fy - 1), BLUE);
        assert_eq!(pixel(&portrait, fx + 1, fy - 6), BLUE);
        // nothing past the sprite, and the background stays clear
        for (x, y) in [
            (fx - 3, fy - 1),
            (fx + 2, fy - 1),
            (fx, fy),
            (fx, fy - 7),
            (0, 0),
        ] {
            assert_eq!(pixel(&portrait, x, y)[3], 0, "({x}, {y})");
        }
    }

    #[test]
    fn a_mirrored_clip_flips_its_texels() {
        let body = sprite(
            image(4, 2, |x, _| if x == 0 { RED } else { BLUE }),
            act(|| clip(0, -1, true), (0, 0)),
        );
        let portrait = compose_portrait(&actor(body, None), 1.0);
        let (fx, fy) = (PORTRAIT_FEET[0] as u32, PORTRAIT_FEET[1] as u32);
        assert_eq!(pixel(&portrait, fx + 1, fy - 1), RED);
        assert_eq!(pixel(&portrait, fx - 2, fy - 1), BLUE);
    }

    #[test]
    fn the_head_goes_over_the_body_at_its_attach_point() {
        // the body's neck is 10 px above the feet; the head's own anchor sits at its clip centre
        let body = sprite(
            image(2, 20, |_, _| RED),
            act(|| clip(0, -10, false), (0, -10)),
        );
        let head = sprite(image(2, 2, |_, _| BLUE), act(|| clip(0, 0, false), (0, 0)));
        let portrait = compose_portrait(&actor(body, Some(head)), 1.0);
        let (fx, fy) = (PORTRAIT_FEET[0] as u32, PORTRAIT_FEET[1] as u32);
        // the 2x2 head is centred on the neck: rows fy-11 and fy-10
        assert_eq!(pixel(&portrait, fx, fy - 11), BLUE);
        assert_eq!(pixel(&portrait, fx - 1, fy - 10), BLUE);
        assert_eq!(pixel(&portrait, fx, fy - 9), RED);
        assert_eq!(pixel(&portrait, fx, fy - 12), RED);
    }

    #[test]
    fn a_half_clear_layer_blends_over_the_one_below() {
        let body = sprite(image(2, 2, |_, _| RED), act(|| clip(0, -1, false), (0, 0)));
        let head = sprite(
            image(2, 2, |_, _| [0, 0, 255, 128]),
            act(|| clip(0, 0, false), (0, 0)),
        );
        let portrait = compose_portrait(&actor(body, Some(head)), 1.0);
        let (fx, fy) = (PORTRAIT_FEET[0] as u32, PORTRAIT_FEET[1] as u32);
        let [r, g, b, a] = pixel(&portrait, fx, fy - 1);
        assert_eq!((g, a), (0, 255));
        assert!(
            (126..=128).contains(&r) && (127..=129).contains(&b),
            "{r} {b}"
        );
    }

    #[test]
    fn a_baby_is_drawn_smaller_about_its_feet() {
        let body = sprite(image(4, 8, |_, _| RED), act(|| clip(0, -4, false), (0, 0)));
        let portrait = compose_portrait(&actor(body, None), 0.5);
        let (fx, fy) = (PORTRAIT_FEET[0] as u32, PORTRAIT_FEET[1] as u32);
        assert_eq!(pixel(&portrait, fx, fy - 4), RED);
        assert_eq!(pixel(&portrait, fx, fy - 5)[3], 0);
        assert_eq!(pixel(&portrait, fx + 2, fy - 1)[3], 0);
    }

    #[test]
    fn the_picture_encodes_as_a_png_of_its_size() {
        let body = sprite(image(2, 2, |_, _| RED), act(|| clip(0, -1, false), (0, 0)));
        let png = compose_portrait(&actor(body, None), 1.0)
            .encode_png()
            .unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), PORTRAIT_SIZE);
        assert_eq!(decoded.get_pixel(100, 189).0, RED);
        assert_eq!(decoded.get_pixel(0, 0).0[3], 0);
    }
}
