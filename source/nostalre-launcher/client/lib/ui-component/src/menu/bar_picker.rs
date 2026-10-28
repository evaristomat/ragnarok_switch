//! Putting a skill or an item on the skill bar from the menu: the bar's four rows of nine drawn over the screen,
//! a slot picked with the arrows or a tap. What it sends is what a drop on the bar sends (`hotkey_bar::assign_*`).
use super::SLOT_BASE_ID;
use super::frame::{self, Hint, Key, Keys, Textures};
use crate::BuildCtx;
use crate::game::hotkey_bar::{
    DOCK_GAP, DOCK_ICON, DOCK_SLOT, assign_item, assign_skill, clear_slot, slot_key,
};
use crate::hud::style;
use nostalre_game::data_table::skill_name_table::format_skill_display_name;
use nostalre_game::event::GameEvent;
use nostalre_game::hotkey::{HOTKEY_COLS, HOTKEY_ROWS, HOTKEY_TOTAL, HotkeySlotContent};
use nostalre_game::skill::{SkillEnum, skill_icon_path};
use nostalre_ui::draw;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const SHADE: [f32; 4] = style::shade(0.88);
/// A slot whose item is not in the bag or whose skill is not learnt: still shown, so it is not put over blind
const ABSENT: [f32; 4] = [1.0, 1.0, 1.0, 0.35];
const KEY_GAP: f32 = 4.0;
const ROW_GAP: f32 = 22.0;

/// What goes on the bar
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ToBar {
    /// An inventory item, as long as that index still holds it
    Item {
        index: u16,
        item_id: u32,
    },
    Skill(SkillEnum),
}

pub(super) struct BarPicker {
    what: ToBar,
    slot: usize,
}

impl BarPicker {
    pub fn new(what: ToBar) -> Self {
        Self { what, slot: 0 }
    }

    /// What is going on the bar, by name; `None` once the item has gone from the bag
    fn name<'a>(&self, ctx: &'a BuildCtx) -> Option<&'a str> {
        match self.what {
            ToBar::Item { index, item_id } => ctx
                .character
                .inventory
                .get_item(index)
                .filter(|i| i.item_id == item_id)
                .map(|i| i.name.as_str()),
            ToBar::Skill(skill) => Some(format_skill_display_name(
                skill,
                ctx.data.skill_name.as_ref(),
            )),
        }
    }

    pub fn footer(&self, ctx: &BuildCtx) -> (String, Vec<Hint>) {
        (
            crate::pt_format!(
                "Put {} on a slot.",
                "Coloque {} em um espaço.",
                self.name(ctx).unwrap_or_default()
            ),
            vec![
                Hint::MOVE,
                Hint::tap("Enter", crate::pt!("put", "colocar"), Key::Enter),
                Hint::tap("Del", crate::pt!("empty slot", "esvaziar"), Key::Remove),
                Hint::CANCEL,
            ],
        )
    }

    /// Draws the bar over `area`; true once the skill or item is on it, or the item is gone
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        if ui.ctx.pad_labels || cfg!(target_os = "horizon") {
            return self.build_compact(ui, ctx, area, keys, textures, events);
        }
        if self.name(ctx).is_none() {
            return true;
        }
        style::fill(ui, area, SHADE);

        let (row, col) = (self.slot / HOTKEY_COLS, self.slot % HOTKEY_COLS);
        let row = if keys.up {
            (row + HOTKEY_ROWS - 1) % HOTKEY_ROWS
        } else if keys.down {
            (row + 1) % HOTKEY_ROWS
        } else {
            row
        };
        let col = if keys.left {
            (col + HOTKEY_COLS - 1) % HOTKEY_COLS
        } else if keys.right {
            (col + 1) % HOTKEY_COLS
        } else {
            col
        };
        self.slot = row * HOTKEY_COLS + col;

        let line = style::line(ui);
        let key_h = ui.atlas.line_height;
        let pitch = DOCK_SLOT + DOCK_GAP;
        let grid_w = HOTKEY_COLS as f32 * pitch - DOCK_GAP;
        let row_h = DOCK_SLOT + KEY_GAP + key_h + ROW_GAP;
        let grid_h = HOTKEY_ROWS as f32 * row_h - ROW_GAP;
        let x0 = area.x + ((area.w - grid_w) / 2.0).floor().max(0.0);
        let y0 = area.y + ((area.h - grid_h - line - 12.0) / 2.0).floor().max(0.0);
        style::heading_rule(
            ui,
            x0,
            y0,
            grid_w,
            crate::pt!("Skill bar", "Barra de habilidades"),
        );
        let top = y0 + line + 12.0;

        let mut put = keys.enter.then_some(self.slot);
        for slot in 0..HOTKEY_TOTAL {
            let (r, c) = (slot / HOTKEY_COLS, slot % HOTKEY_COLS);
            let rect = Rect::new(
                x0 + c as f32 * pitch,
                top + r as f32 * row_h,
                DOCK_SLOT,
                DOCK_SLOT,
            );
            let resp = ui.interact(WidgetId(SLOT_BASE_ID + slot as u32), rect);
            if slot == self.slot {
                style::push_white(
                    ui,
                    draw::rounded_rect(
                        rect.x - 2.0,
                        rect.y - 2.0,
                        rect.w + 4.0,
                        rect.h + 4.0,
                        7.0,
                        style::ACCENT,
                    ),
                );
            }
            style::slot(ui, rect, resp.hovered());
            if let Some((path, tint)) = slot_icon(ctx, ctx.character.hotkeys.get_slot(slot)) {
                textures.want(&path);
                let pad = ((DOCK_SLOT - DOCK_ICON) / 2.0).floor();
                let icon = Rect::new(rect.x + pad, rect.y + pad, DOCK_ICON, DOCK_ICON);
                frame::image(ui, icon, path, tint);
            }
            if let Some(code) = slot_key(slot) {
                let label = ui.ctx.key_labels.display(code);
                let w = ui.atlas.measure_text(&label);
                let x = rect.x + ((DOCK_SLOT - w) / 2.0).floor();
                let baseline = rect.y + DOCK_SLOT + KEY_GAP + ui.atlas.ascent;
                style::outlined(ui, x, baseline, &label, style::DIM);
            }
            if resp.clicked() {
                self.slot = slot;
                put = Some(slot);
            }
        }

        if keys.remove {
            events.push(clear_slot(ctx.character, self.slot));
        }
        let Some(slot) = put else {
            return false;
        };
        let character = &mut *ctx.character;
        let event = match self.what {
            ToBar::Item { index, .. } => assign_item(character, slot, index),
            ToBar::Skill(skill) => {
                assign_skill(character, ctx.data.skill_use_level.as_ref(), slot, skill)
            }
        };
        events.extend(event);
        true
    }
}

/// What a slot holds, as its icon: faint when the bar could not use it now (`HotkeyBarWindow` leaves those
/// out; here they show, so a slot is never put over unseen)
fn slot_icon(ctx: &BuildCtx, content: HotkeySlotContent) -> Option<(String, [f32; 4])> {
    let c = &*ctx.character;
    let tint = |there: bool| if there { [1.0; 4] } else { ABSENT };
    match content {
        HotkeySlotContent::Skill { skill, .. } => {
            Some((skill_icon_path(skill), tint(c.skills.knows(skill))))
        }
        HotkeySlotContent::Item { item_id } => {
            let held = c.inventory.find_by_item_id(item_id).is_some();
            Some((ctx.data.item_icon_path(item_id)?, tint(held)))
        }
        HotkeySlotContent::Empty => None,
    }
}

impl BarPicker {
    fn build_compact(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        use crate::hud::compact as s;
        use nostalre_game::hotkey::{PAD_SET_SLOTS, PAD_SETS};
        let Some(name) = self.name(ctx) else {
            return true;
        };
        s::panel(ui, area);
        let title = format!("Atalho · {name}");
        s::text(
            ui,
            area.x + 12.0,
            s::baseline(ui, area.y + 8.0),
            &s::fit(ui, &title, area.w - 24.0),
            s::TEXT,
        );
        let mut set = self.slot / PAD_SET_SLOTS;
        if keys.prev_tab {
            set = (set + usize::from(PAD_SETS) - 1) % usize::from(PAD_SETS);
        }
        if keys.next_tab {
            set = (set + 1) % usize::from(PAD_SETS);
        }
        let mut local = self.slot % PAD_SET_SLOTS;
        let row = local / 4;
        let col = local % 4;
        if keys.up {
            local = (row + 2) % 3 * 4 + col;
        }
        if keys.down {
            local = (row + 1) % 3 * 4 + col;
        }
        if keys.left {
            local = row * 4 + (col + 3) % 4;
        }
        if keys.right {
            local = row * 4 + (col + 1) % 4;
        }
        self.slot = set * PAD_SET_SLOTS + local;
        let pitch = 40.0;
        let size = 30.0;
        let x = area.x + (area.w - 160.0) * 0.5;
        let y = area.y + 29.0;
        let mut put = keys.enter.then_some(self.slot);
        for group in 0..3 {
            let ry = y + group as f32 * 40.0;
            s::right(
                ui,
                x - 10.0,
                s::baseline(ui, ry + 10.0),
                ["L", "L+R", "R"][group],
                s::DIM,
            );
            for face in 0..4 {
                let index = set * 12 + group * 4 + face;
                let r = Rect::new(x + face as f32 * pitch, ry, size, size);
                s::slot(ui, r, index == self.slot);
                let resp = ui.interact(WidgetId(SLOT_BASE_ID + (group * 4 + face) as u32), r);
                if let Some((path, tint)) = slot_icon(ctx, ctx.character.hotkeys.get_slot(index)) {
                    textures.want(&path);
                    frame::image(ui, Rect::new(r.x + 4.0, r.y + 2.0, 22.0, 22.0), path, tint);
                }
                s::right(
                    ui,
                    r.x + r.w - 2.0,
                    r.y + r.h - 2.0,
                    ["Y", "X", "B", "A"][face],
                    s::ACCENT,
                );
                if resp.clicked() {
                    put = Some(index);
                }
            }
        }
        s::center(
            ui,
            area.x + area.w * 0.5,
            s::baseline(ui, area.y + area.h - 20.0),
            &format!("L/R  Conjunto {}/{}", set + 1, PAD_SETS),
            s::DIM,
        );
        if keys.remove {
            events.push(clear_slot(ctx.character, self.slot));
        }
        let Some(slot) = put else {
            return false;
        };
        let event = match self.what {
            ToBar::Item { index, .. } => assign_item(ctx.character, slot, index),
            ToBar::Skill(skill) => assign_skill(
                ctx.character,
                ctx.data.skill_use_level.as_ref(),
                slot,
                skill,
            ),
        };
        events.extend(event);
        true
    }
}
