//! The frameless HUD (`docs/nostalre-hud.md`): text and bars straight on the world, laid out as FF14
//! places things. `HudLayout::Classic` keeps kRO's windows instead.
mod camera;
pub(crate) mod compact;
mod controller;
mod corner;
mod party;
mod player;
pub mod style;
mod target;
mod touch;

use crate::BuildCtx;
use crate::game::hotkey_bar::{DOCK_W, HotkeyDock};
pub use camera::CameraReadout;
use nostalre_game::data_table::DataTable;
pub use nostalre_game::display::HudLayout;
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;
pub use touch::{LEFT_COLUMN_H as TOUCH_LEFT_COLUMN_H, SlotFace, TouchInput, TouchSlots};

/// A cast in progress, read from the caster's entity
#[derive(Debug, Clone, PartialEq)]
pub struct CastInfo {
    pub name: String,
    pub remaining: f32,
    pub total: f32,
}

impl CastInfo {
    pub fn progress(&self) -> f32 {
        if self.total <= 0.0 {
            1.0
        } else {
            (1.0 - self.remaining / self.total).clamp(0.0, 1.0)
        }
    }
}

/// Another unit as the HUD names it: its HP as a ratio when the server has told us
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub name: String,
    pub hp: Option<f32>,
}

/// The HUD's target (`docs/nostalre-hud.md`, "The target")
#[derive(Debug, Clone, PartialEq)]
pub struct TargetInfo {
    /// Its id: a new target starts its HP trail afresh
    pub id: u32,
    pub unit: Unit,
    pub level: Option<u16>,
    pub boss: bool,
    pub cast: Option<CastInfo>,
    /// Whom it is attacking or casting on
    pub target: Option<Unit>,
}

/// Where the player stands
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub map: String,
    pub x: u16,
    pub y: u16,
}

/// A map's name as the player reads it, else its file name
pub fn place_name<'a>(data: &'a DataTable, map: &'a str) -> &'a str {
    data.map_name
        .as_ref()
        .and_then(|t| t.display_name(map))
        .unwrap_or(map)
}

/// What the client knows that `BuildCtx` does not carry, set each frame before `build`
#[derive(Debug, Default)]
pub struct HudInput {
    /// Round trip to the server, once the clock is synced
    pub ping_ms: Option<u32>,
    pub place: Option<Place>,
    /// Where the chat was drawn last frame, for the shade behind it
    pub chat: Option<Rect>,
    pub player_cast: Option<CastInfo>,
    pub target: Option<TargetInfo>,
    /// Monsters whose last attack went at the player
    pub attackers: Vec<Unit>,
    /// The camera, while a controller or the on-screen controls move it
    pub camera: Option<CameraReadout>,
    /// What the on-screen controls' slot buttons show, while they are on screen
    pub touch_slots: TouchSlots,
    /// Held modifier: L, L+R or R. No modifier shows the two outer banks.
    pub controller_group: Option<u8>,
}

/// Native player head drawn by the scene renderer at this exact UI layer.
#[derive(Debug, Clone, Copy)]
pub struct PortraitPlacement {
    pub rect: Rect,
    pub insert_index: usize,
}

/// The windows the HUD draws in its own places: the skill rows (`hotkey_rows` of them), the minimap; a tap on
/// its menu line
#[derive(Debug, Clone, Copy)]
pub struct HudPlacement {
    pub hotkeys: HotkeyDock,
    pub minimap: Rect,
    /// "Esc  Menu" was tapped
    pub menu_tapped: bool,
    pub portrait: Option<PortraitPlacement>,
}

#[derive(Debug, Default)]
pub struct Hud {
    pub layout: HudLayout,
    pub input: HudInput,
    /// Draw the on-screen controls while no controller was used last (a handheld)
    pub touch_controls: bool,
    touch: touch::TouchControls,
    touch_input: TouchInput,
    hp_trail: style::Trail,
    target_trail: style::Trail,
    portrait: Option<PortraitPlacement>,
}

impl Hud {
    pub fn portrait(&self) -> Option<PortraitPlacement> {
        self.portrait
    }

    pub fn is_modern(&self) -> bool {
        self.layout == HudLayout::Modern
    }

    pub fn uses_controller(&self, pad_labels: bool) -> bool {
        self.is_modern() && (pad_labels || cfg!(target_os = "horizon"))
    }

    /// The on-screen controls are drawn: a frameless HUD on a touch screen, no controller used last
    pub fn shows_touch(&self, pad_labels: bool) -> bool {
        self.is_modern() && self.touch_controls && !pad_labels
    }

    /// What the on-screen controls asked in the last frame the HUD was drawn; nothing once it is not
    pub fn touch_input(&self) -> TouchInput {
        self.touch_input
    }

    /// Where the on-screen controls were last drawn: a finger landing there is theirs, not the mouse
    pub fn touch_zones(&self) -> &[Rect] {
        self.touch.zones()
    }

    /// The HUD is not drawn this frame (the menu covers it, the classic layout): no control stays held
    pub fn hidden(&mut self) {
        self.touch_input = TouchInput::default();
        self.touch.hide();
        self.portrait = None;
    }

    /// Draws the HUD and returns where the windows it places go
    pub fn build(&mut self, ui: &mut UiFrame, ctx: &BuildCtx, hotkey_rows: usize) -> HudPlacement {
        if self.uses_controller(ui.ctx.pad_labels) {
            self.hidden();
            let placement = controller::build(ui, ctx, &self.input, &mut self.target_trail);
            self.portrait = placement.portrait;
            return placement;
        }
        self.portrait = None;
        if let Some(chat) = self.input.chat {
            style::corner_scrim(ui, chat);
        }
        party::build(ui, ctx, &self.input.attackers);
        let plate = target::build(ui, self.input.target.as_ref(), &mut self.target_trail);
        let minimap = corner::build(ui, ctx, &self.input);
        let menu_tapped = corner::wallet(ui, ctx);
        let hotkeys = player::build(ui, ctx, &self.input, hotkey_rows, &mut self.hp_trail);
        if let Some(readout) = &self.input.camera {
            camera::readout(ui, readout, plate);
        }
        // the skill rows and the gauges under them, down to the screen's bottom
        let dock_h = ui.ctx.screen_height - hotkeys.y;
        let dock = Rect::new(hotkeys.x, hotkeys.y, DOCK_W, dock_h);
        self.touch_input = TouchInput::default();
        if self.shows_touch(ui.ctx.pad_labels) {
            let slots = &self.input.touch_slots;
            self.touch_input = self.touch.build(ui, minimap, dock, slots);
        } else if self.touch_controls {
            self.touch.rest(ui, minimap, dock);
        } else {
            self.touch.hide();
        }
        HudPlacement {
            hotkeys,
            minimap,
            menu_tapped,
            portrait: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiding_the_hud_discards_the_previous_portrait_layer() {
        let mut hud=Hud::default();
        hud.portrait=Some(PortraitPlacement{rect:Rect::new(10.0,10.0,44.0,44.0),insert_index:7});
        assert!(hud.portrait().is_some());
        hud.hidden();
        assert!(hud.portrait().is_none());
    }

    #[test]
    fn a_cast_fills_as_its_time_runs_out() {
        let cast = |remaining, total| CastInfo {
            name: String::new(),
            remaining,
            total,
        };
        assert_eq!(cast(1.0, 4.0).progress(), 0.75);
        assert_eq!(cast(5.0, 4.0).progress(), 0.0);
        assert_eq!(cast(0.0, 0.0).progress(), 1.0);
    }
}
