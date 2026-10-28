//! The equipment window's Damage Indicator tab, as the 2025 exe builds it (0x8a4b30) and draws it (0x896e10): the
//! damage skins as cells on `bg_damage.bmp`, four radios for the way damage numbers drift, Apply and Close. A
//! click picks a cell (0x8b9b00); Apply makes the picks the skin and direction in use (0x8c0291), which kRO's client
//! keeps in `OptionInfo.lua` and tells the server nothing about.
use crate::helper::window_chrome::{draw_textured_quad, text_color};
use nostalre_game::damage_number::DamageDirection;
use nostalre_game::damage_skin::DamageSkin;
use nostalre_resources::ui as art;
use nostalre_ui::frame::{ButtonTextures, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The tab's art, from the window's top (`+0xf4` + 0xe)
const BG_Y: f32 = 34.0;
/// The cells: the first one's x, the step to the next, their top, their size; four in view (0x896e10, 0x8b9b00)
const CELL_X: f32 = 6.0;
const CELL_STEP: f32 = 68.0;
const CELL_Y: f32 = 58.0;
const CELL_SIZE: (f32, f32) = (64.0, 92.0);
const CELLS_SHOWN: usize = 4;
/// A cell's name, centred in its width, its shadow a pixel right and down; its icon
const NAME_Y: f32 = 68.0;
const NAME_COLOR: [f32; 4] = [50.0 / 255.0, 50.0 / 255.0, 50.0 / 255.0, 1.0];
const NAME_SHADOW: [f32; 4] = [186.0 / 255.0, 186.0 / 255.0, 186.0 / 255.0, 1.0];
const ICON_OFFSET: (f32, f32) = (14.0, 90.0);
const ICON_SIZE: f32 = 36.0;
/// The radios in kRO's order (Default, ↗, ↑, ↖ in its own table; the English one swaps Left and Right): x, their
/// row, their box (0x8a5dcd)
const RADIO_X: [f32; 4] = [8.0, 88.0, 148.0, 208.0];
const RADIO_Y: f32 = 40.0;
const RADIO_SIZE: (f32, f32) = (60.0, 12.0);
const RADIO_BOX: f32 = 10.0;
/// The label beside the box, as the window's check box has it (the rig, 2026-10-20)
const RADIO_LABEL_X: f32 = 13.0;
const RADIO_LABEL_BASELINE: f32 = 8.0;
const RADIO_LABELS: [&str; 4] = ["Default", "Right", "Center", "Left"];
/// Apply and Close in the footer (0xa0 and 0xdc, 0xa7)
const APPLY_POS: (f32, f32) = (160.0, 167.0);
const CLOSE_POS: (f32, f32) = (220.0, 167.0);
const BUTTON_SIZE: (f32, f32) = (42.0, 20.0);
const APPLY_TEX: ButtonTextures = ButtonTextures {
    normal: art::BTN_APPLY,
    hover: art::BTN_APPLY_A,
    pressed: art::BTN_APPLY_B,
};
const CLOSE_TEX: ButtonTextures = ButtonTextures {
    normal: art::BTN_CLOSE,
    hover: art::BTN_CLOSE_A,
    pressed: art::BTN_CLOSE_B,
};

pub const TEXTURES: [&str; 12] = [
    art::SHOW_DAMAGE_BG,
    art::SHOW_DAMAGE_CELL,
    art::SHOW_DAMAGE_CELL_APPLIED,
    art::SHOW_DAMAGE_CELL_PICKED,
    art::TIPBOX_CHECKBOX,
    art::TIPBOX_CHECKBOX_CHECK,
    art::BTN_APPLY,
    art::BTN_APPLY_A,
    art::BTN_APPLY_B,
    art::BTN_CLOSE,
    art::BTN_CLOSE_A,
    art::BTN_CLOSE_B,
];

pub enum DamageTabAction {
    Apply {
        skin: u32,
        direction: DamageDirection,
    },
    Close,
}

/// The picks on the tab, made anew from the skin and direction in use each time the window opens (the ctor
/// 0x88d740 copies them)
#[derive(Default)]
pub struct DamageIndicatorTab {
    picked: usize,
    direction: DamageDirection,
}

impl DamageIndicatorTab {
    pub fn reset(&mut self, applied: u32, direction: DamageDirection) {
        self.picked = applied as usize;
        self.direction = direction;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        base_id: u32,
        win_x: f32,
        win_y: f32,
        skins: &[DamageSkin],
        applied: u32,
        grf: bool,
    ) -> Option<DamageTabAction> {
        draw_textured_quad(ui, win_x, win_y + BG_Y, 280.0, 133.0, art::SHOW_DAMAGE_BG);
        for (k, skin) in skins.iter().take(CELLS_SHOWN).enumerate() {
            let x = win_x + CELL_X + k as f32 * CELL_STEP;
            let y = win_y + CELL_Y;
            let is_applied = k == applied as usize;
            let frame = if is_applied {
                art::SHOW_DAMAGE_CELL_APPLIED
            } else if k == self.picked {
                art::SHOW_DAMAGE_CELL_PICKED
            } else {
                art::SHOW_DAMAGE_CELL
            };
            draw_textured_quad(ui, x, y, CELL_SIZE.0, CELL_SIZE.1, frame);
            let name_x = x + ((CELL_SIZE.0 - ui.atlas.measure_text(&skin.name)) * 0.5).floor();
            let baseline = win_y + NAME_Y + ui.atlas.ascent;
            let text = |ui: &mut UiFrame, x: f32, y: f32, color| {
                if is_applied {
                    ui.text_bold(x, y, &skin.name, color);
                } else {
                    ui.text(x, y, &skin.name, color);
                }
            };
            text(ui, name_x + 1.0, baseline + 1.0, NAME_SHADOW);
            text(ui, name_x, baseline, NAME_COLOR);
            draw_textured_quad(
                ui,
                x + ICON_OFFSET.0,
                win_y + ICON_OFFSET.1,
                ICON_SIZE,
                ICON_SIZE,
                // The table's path, owned: copied for the frame while this tab is open
                skin.icon.clone(),
            );
            let response = ui.interact(
                WidgetId(base_id + k as u32),
                Rect::new(x, y, CELL_SIZE.0, CELL_SIZE.1),
            );
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }
            if response.clicked() {
                self.picked = k;
            }
        }
        for (i, direction) in DamageDirection::ALL.into_iter().enumerate() {
            let (x, y) = (win_x + RADIO_X[i], win_y + RADIO_Y);
            let tex = if direction == self.direction {
                art::TIPBOX_CHECKBOX_CHECK
            } else {
                art::TIPBOX_CHECKBOX
            };
            draw_textured_quad(ui, x, y, RADIO_BOX, RADIO_BOX, tex);
            ui.text(
                x + RADIO_LABEL_X,
                y + RADIO_LABEL_BASELINE,
                RADIO_LABELS[i],
                text_color(grf),
            );
            let response = ui.interact(
                WidgetId(base_id + 4 + i as u32),
                Rect::new(x, y, RADIO_SIZE.0, RADIO_SIZE.1),
            );
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }
            if response.clicked() {
                self.direction = direction;
            }
        }
        let button =
            |ui: &mut UiFrame, id: u32, (x, y): (f32, f32), tex: &ButtonTextures, label| {
                let rect = Rect::new(win_x + x, win_y + y, BUTTON_SIZE.0, BUTTON_SIZE.1);
                ui.button(WidgetId(id), rect, tex, label).clicked()
            };
        if button(ui, base_id + 8, APPLY_POS, &APPLY_TEX, "Apply") {
            return Some(DamageTabAction::Apply {
                skin: self.picked as u32,
                direction: self.direction,
            });
        }
        if button(ui, base_id + 9, CLOSE_POS, &CLOSE_TEX, crate::pt!("Close", "Fechar")) {
            return Some(DamageTabAction::Close);
        }
        None
    }
}
