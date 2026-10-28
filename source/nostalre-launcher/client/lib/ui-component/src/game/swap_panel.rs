//! The Swap Equipment panel under the equipment window (`UISwapEquipmentWnd`, the 2025 exe's 0x7f7fe0): the swap set
//! in the tab's slots on its own art, each empty one showing the item worn there faded, CHANGE and Cancel. An
//! inventory item dropped on it goes into the set, a worn one dropped from the window swaps alone; a slot's item
//! double-clicked comes out of the set, with Ctrl swaps alone, dragged out to the inventory comes out, onto the
//! window swaps alone (0x7f8410, 0x7f98a0; `docs/nostalre-kro-exe-equip-window.md`, "Swap Equipment").
use super::equipment_window::{EQ_SWAP_DRAG_ID, EQ_WINDOW_ID, EquipSlot, draw_slot_item};
use super::inventory_window::INV_WINDOW_ID;
use crate::helper::window_chrome::draw_textured_quad;
use nostalre_game::char_name::CharNameCache;
use nostalre_game::data_table::DataTable;
use nostalre_game::display_name::format_equipment_display_name_colored;
use nostalre_game::equip_switch::EquipSwitch;
use nostalre_game::event::GameEvent;
use nostalre_game::inventory::InventoryData;
use nostalre_resources::ui as art;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The panel's size and its place under the window's slots (0x8c0496: 0x118 × 0xb3; Move(0, 0xa7))
pub(super) const PANEL_SIZE: (f32, f32) = (280.0, 179.0);
pub(super) const PANEL_Y: f32 = 167.0;
const ICON: f32 = 24.0;
/// A slot's icon and name in the panel by column, its rows' icon tops and names' lines (0x7f8810, table 0x10274a0:
/// the top row's name sits 8 under its icon, the others 6); the names' first baseline is two under kRO's line
const ICON_X: [f32; 2] = [6.0, 250.0];
const NAME_X: [f32; 2] = [34.0, 173.0];
const ICON_Y: [f32; 5] = [22.0, 48.0, 74.0, 100.0, 126.0];
const NAME_Y: [f32; 5] = [30.0, 54.0, 80.0, 106.0, 132.0];
const NAME_BASELINE_DROP: f32 = 2.0;
/// A slot answers in its whole row: x 6 to 0x69 on the left, 0xaa to the edge less 6 on the right (0x7f8e80)
const ROW_X: [(f32, f32); 2] = [(6.0, 105.0), (170.0, 274.0)];
const ROW_H: f32 = 26.0;
/// The ammunition slot in the middle (0x7f97a6)
const AMMO_ICON: (f32, f32) = (116.0, 27.0);
/// The worn item standing in for an empty slot, faded (0xa1d530 after 0x7129c0(1); how much is a rig check)
const STAND_IN_ALPHA: f32 = 0.5;
/// The title, centred on 0x8c (0x7f9652); CHANGE and Cancel (0x7f91e0, 0x7f93a1)
const TITLE_CENTRE: (f32, f32) = (140.0, 4.0);
const TITLE_COLOR: [f32; 4] = [75.0 / 255.0, 35.0 / 255.0, 20.0 / 255.0, 1.0];
const CHANGE_RECT: (f32, f32, f32, f32) = (81.0, 155.0, 118.0, 20.0);
const CHANGE_COLOR: [f32; 4] = [66.0 / 255.0, 40.0 / 255.0, 23.0 / 255.0, 1.0];
const CHANGE_WAIT_COLOR: [f32; 4] = [150.0 / 255.0, 150.0 / 255.0, 150.0 / 255.0, 1.0];
const CANCEL_RECT: (f32, f32, f32, f32) = (224.0, 155.0, 50.0, 20.0);

pub(super) const TEXTURES: [&str; 6] = [
    art::SWAP_BG_GENERAL,
    art::SWAP_BG_COSTUME,
    art::SWAP_CHANGE,
    art::SWAP_CHANGE_OVER,
    art::SWAP_CHANGE_PRESS,
    art::SWAP_CHANGE_DISABLE,
];

pub(super) enum SwapPanelAction {
    Event(GameEvent),
    /// Cancel: the panel shuts and stops opening with the window (0x7fa29b)
    Cancel,
}

/// What the panel needs of the character
pub(super) struct SwapView<'a> {
    pub(super) inventory: &'a InventoryData,
    pub(super) set: &'a EquipSwitch,
    pub(super) data: &'a DataTable,
    pub(super) producers: &'a CharNameCache,
    pub(super) text_color: [f32; 4],
}

fn text_centred(ui: &mut UiFrame, centre_x: f32, top: f32, text: &str, color: [f32; 4]) {
    let x = centre_x - (ui.atlas.measure_text(text) * 0.5).floor();
    ui.text(x, top + ui.atlas.ascent, text, color);
}

/// The panel at (`x`, `y`) for the tab's `slots`; `costume` picks its art
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    ui: &mut UiFrame,
    base_id: u32,
    x: f32,
    y: f32,
    slots: &[EquipSlot],
    costume: bool,
    view: &SwapView,
) -> Vec<SwapPanelAction> {
    let mut actions = Vec::new();
    let (w, h) = PANEL_SIZE;
    let bg = if costume {
        art::SWAP_BG_COSTUME
    } else {
        art::SWAP_BG_GENERAL
    };
    draw_textured_quad(ui, x, y, w, h, bg);
    text_centred(
        ui,
        x + TITLE_CENTRE.0,
        y + TITLE_CENTRE.1,
        "Swap Equipment",
        TITLE_COLOR,
    );

    for (i, slot) in slots.iter().enumerate() {
        let mask = InventoryData::slot_mask(slot.location);
        let (icon_rect, row_rect, name) = match slot.col {
            0 | 1 => {
                let col = slot.col as usize;
                let row = slot.row as usize;
                let (left, right) = ROW_X[col];
                (
                    Rect::new(x + ICON_X[col], y + ICON_Y[row], ICON, ICON),
                    Rect::new(x + left, y + ICON_Y[row] - 1.0, right - left, ROW_H),
                    Some((x + NAME_X[col], y + NAME_Y[row] + NAME_BASELINE_DROP)),
                )
            }
            _ => {
                let rect = Rect::new(x + AMMO_ICON.0, y + AMMO_ICON.1, ICON, ICON);
                (rect, rect, None)
            }
        };
        let set_item = view
            .set
            .item_in(mask)
            .and_then(|index| view.inventory.get_item(index));
        let (item, alpha) = match set_item {
            Some(item) => (Some(item), 1.0),
            // A worn item stands in only where no set item takes any of its places
            None => match view.inventory.equipped_in_slot(slot.location) {
                Some(worn) if view.set.item_in(worn.wear_state).is_none() => {
                    (Some(worn), STAND_IN_ALPHA)
                }
                _ => (None, 1.0),
            },
        };
        if let Some(item) = item {
            let display_name = format_equipment_display_name_colored(
                item,
                view.data,
                view.producers,
                view.text_color,
            );
            draw_slot_item(
                ui,
                item,
                &display_name,
                icon_rect,
                name,
                view.text_color,
                alpha,
            );
        }
        let response = ui.interact(WidgetId(base_id + i as u32), row_rect);
        let Some(item) = set_item else {
            continue;
        };
        if response.clicked() {
            ui.drag_source(
                EQ_SWAP_DRAG_ID,
                item.index as usize,
                item.icon_path(),
                (ICON, ICON),
            );
        }
        if response.right_clicked() {
            actions.push(SwapPanelAction::Event(GameEvent::ShowItemInfo {
                index: item.index,
            }));
        }
        if response.double_clicked() {
            let event = if ui.ctx.ctrl_pressed {
                GameEvent::RequestEquipSwitchSingle { index: item.index }
            } else {
                GameEvent::RequestEquipSwitchRemove { index: item.index }
            };
            actions.push(SwapPanelAction::Event(event));
        }
    }

    let waiting = view.set.change_wait_secs(ui.ctx.now_ms).is_some();
    let (cx, cy, cw, ch) = CHANGE_RECT;
    let change_rect = Rect::new(x + cx, y + cy, cw, ch);
    let change = ui.interact(WidgetId(base_id + 20), change_rect);
    let (change_tex, change_color) = if waiting {
        (art::SWAP_CHANGE_DISABLE, CHANGE_WAIT_COLOR)
    } else if change.hovered() && (ui.ctx.mouse_clicked || ui.ctx.mouse_down) {
        (art::SWAP_CHANGE_PRESS, CHANGE_COLOR)
    } else if change.hovered() {
        (art::SWAP_CHANGE_OVER, CHANGE_COLOR)
    } else {
        (art::SWAP_CHANGE, CHANGE_COLOR)
    };
    if change.hovered() {
        ui.any_interactive_hovered = true;
    }
    draw_textured_quad(ui, change_rect.x, change_rect.y, cw, ch, change_tex);
    let change_top = change_rect.y + ((ch - ui.atlas.line_height) * 0.5).floor();
    text_centred(
        ui,
        change_rect.x + cw * 0.5,
        change_top,
        "CHANGE",
        change_color,
    );
    // Pressed during the wait too: the client then says how long is left (0xc928fe)
    if change.clicked() {
        actions.push(SwapPanelAction::Event(GameEvent::RequestEquipSwitch));
    }

    let (kx, ky, kw, kh) = CANCEL_RECT;
    let cancel_rect = Rect::new(x + kx, y + ky, kw, kh);
    let cancel = ui.interact(WidgetId(base_id + 21), cancel_rect);
    if cancel.hovered() {
        ui.any_interactive_hovered = true;
    }
    let cancel_top = cancel_rect.y + ((kh - ui.atlas.line_height) * 0.5).floor();
    text_centred(
        ui,
        cancel_rect.x + kw * 0.5,
        cancel_top,
        crate::pt!("Cancel", "Cancelar"),
        view.text_color,
    );
    if cancel.clicked() {
        actions.push(SwapPanelAction::Cancel);
    }

    let panel_rect = Rect::new(x, y, w, h);
    if let Some((source, index)) = ui.drop_zone(panel_rect)
        && let Some(item) = view.inventory.get_item(index as u16)
    {
        let event = match source {
            INV_WINDOW_ID if item.is_equipment() => Some(GameEvent::RequestEquipSwitchAdd {
                index: item.index,
                location: item.equip_location(),
            }),
            EQ_WINDOW_ID if !item.is_ammunition() => {
                Some(GameEvent::RequestEquipSwitchSingle { index: item.index })
            }
            _ => None,
        };
        actions.extend(event.map(SwapPanelAction::Event));
    }
    actions
}
