use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::character::ActiveStatus;
use nostalre_game::event::GameEvent;
use nostalre_game::status_icon::{StatusCategory, StatusIconInfo, status_icon_info};
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const STATUS_ICON_BAR_WINDOW_ID: WidgetId = WidgetId(1900);

const ICON: f32 = 24.0;
const GAP: f32 = 4.0;
const RIGHT_MARGIN: f32 = 20.0;
const TOP_Y: f32 = 2.0 + 128.0 + 14.0;
const BOTTOM_MARGIN: f32 = 60.0;
const FINAL_WINDOW_MS: u64 = 60_000;

const WEDGE_NORMAL: [f32; 4] = [1.0, 1.0, 1.0, 0.65];
const WEDGE_FINAL: [f32; 4] = [1.0, 0.59, 0.20, 0.65];

#[derive(Default)]
pub struct StatusIconBarWindow {
    pub has_grf_textures: bool,
}

impl StatusIconBarWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: true,
        }
    }
}

fn category_color(cat: StatusCategory) -> [f32; 4] {
    match cat {
        StatusCategory::Buff => [0.61, 0.79, 0.61, 1.0],
        StatusCategory::Debuff => [0.98, 0.39, 0.39, 1.0],
        StatusCategory::Toggle => [0.75, 0.75, 0.98, 1.0],
    }
}

impl Window for StatusIconBarWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        Vec::new()
    }
}

impl InGameWindow for StatusIconBarWindow {
    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let character = &mut *ctx.character;
        let _data = ctx.data;
        let now = ui.ctx.now_ms;
        let mut x = ui.ctx.screen_width - RIGHT_MARGIN - ICON;
        let mut y = TOP_Y;

        for (i, status) in character.active_statuses.iter().enumerate() {
            let Some(info) = status_icon_info(status.efst) else {
                continue;
            };

            if y + ICON > ui.ctx.screen_height - BOTTOM_MARGIN {
                y = TOP_Y;
                x -= ICON + GAP;
            }
            draw_status_icon(ui, Rect::new(x, y, ICON, ICON), status, info, now, i);

            y += ICON + GAP;
        }

        Vec::new()
    }
}

/// One status: its icon (a category colour while the icon loads), the sweep of its time, its tooltip
/// on hover. `i` keeps each icon's hover id apart
pub fn draw_status_icon(
    ui: &mut UiFrame,
    rect: Rect,
    status: &ActiveStatus,
    info: &StatusIconInfo,
    now: u64,
    i: usize,
) {
    let (x, y, size) = (rect.x, rect.y, rect.w);
    if status.icon_loaded {
        let path = nostalre_resources::texture::effect::named(&info.icon);
        let (v, idx) = draw::quad_vertices(x, y, size, size, [1.0, 1.0, 1.0, 1.0]);
        ui.draw_calls.push(DrawCall {
            vertices: v.into(),
            indices: idx.into(),
            texture: TextureRef::Named(path.into()),
        });
    } else {
        let mut c = category_color(info.category);
        c[3] = 0.55;
        let (v, idx) = draw::quad_vertices(x, y, size, size, c);
        ui.draw_calls.push(DrawCall {
            vertices: v.into(),
            indices: idx.into(),
            texture: TextureRef::White,
        });
    }

    if let Some(end) = status.end_ms {
        let remaining = end.saturating_sub(now);
        let (color, perc) = if remaining > FINAL_WINDOW_MS {
            let span = (end - FINAL_WINDOW_MS)
                .saturating_sub(status.start_ms)
                .max(1);
            (
                WEDGE_NORMAL,
                (now.saturating_sub(status.start_ms) as f32 / span as f32).clamp(0.0, 1.0),
            )
        } else {
            (
                WEDGE_FINAL,
                (1.0 - remaining as f32 / FINAL_WINDOW_MS as f32).clamp(0.0, 1.0),
            )
        };
        if perc > 0.0 {
            let (v, idx) = draw::square_wedge_vertices(
                x + size * 0.5,
                y + size * 0.5,
                size * 0.5,
                -std::f32::consts::FRAC_PI_2,
                perc * std::f32::consts::TAU,
                color,
            );
            ui.draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: idx.into(),
                texture: TextureRef::White,
            });
        }
    }

    let resp = ui.interact(WidgetId(STATUS_ICON_BAR_WINDOW_ID.0 + 1 + i as u32), rect);
    if resp.hovered() {
        ui.any_interactive_hovered = true;
        draw_tooltip(ui, rect, info);
    }
}

fn draw_tooltip(ui: &mut UiFrame, rect: Rect, info: &StatusIconInfo) {
    let mut lines: Vec<(&str, [f32; 4])> = vec![(info.name, category_color(info.category))];
    if !info.description.is_empty() {
        for line in info.description.split('\n') {
            lines.push((line, [0.85, 0.85, 0.85, 1.0]));
        }
    }

    let line_h = ui.atlas.line_height;
    let pad = 6.0;
    let max_w = lines
        .iter()
        .map(|(t, _)| ui.atlas.measure_text(t))
        .fold(0.0f32, f32::max);
    let box_w = max_w + pad * 2.0;
    let box_h = lines.len() as f32 * line_h + pad * 2.0;

    let tx = (rect.x - box_w - 6.0).max(2.0);
    let ty = rect.y.min(ui.ctx.screen_height - box_h - 2.0);

    let (v, idx) = draw::quad_vertices(tx, ty, box_w, box_h, [0.0, 0.0, 0.0, 0.85]);
    ui.tooltip_draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: idx.into(),
        texture: TextureRef::White,
    });

    let mut text_y = ty + pad + line_h;
    for (text, color) in &lines {
        let (v, i) = draw::text_vertices(text, tx + pad, text_y, *color, ui.atlas);
        if !v.is_empty() {
            ui.tooltip_draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: TextureRef::FontAtlas,
            });
        }
        text_y += line_h;
    }
}
