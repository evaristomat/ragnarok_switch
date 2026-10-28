//! The refine window (rAthena's refine UI, opened by a refiner NPC): kRO's forge. An equipment goes in from the
//! inventory (double-click or drag), the server lists the materials that refine it, the player picks one (and
//! the Blacksmith Blessing where the level takes it) and refines; the forge plays the hammer, then the result.
//! Layout and frame timing as kRO's 2022 client shows them (`refining_renewal/` art).

use super::inventory_window::INV_WINDOW_ID;
use crate::helper::window_chrome::{draw_footer, draw_textured_quad, draw_titlebar, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::event::GameEvent;
use nostalre_game::refine::{BLACKSMITH_BLESSING, MATERIAL_SLOTS, RefineBlock, refinable};
use nostalre_resources::ui::refining as art;
use nostalre_ui::frame::{ButtonTextures, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const REFINE_WINDOW_ID: WidgetId = WidgetId(6000);
const MATERIAL_BASE: u32 = 6001;
const BLESSING_ID: WidgetId = WidgetId(6006);
const ITEM_SLOT_ID: WidgetId = WidgetId(6007);
const REFINE_ID: WidgetId = WidgetId(6008);
const BACK_ID: WidgetId = WidgetId(6009);
const AGAIN_ID: WidgetId = WidgetId(6010);
const CANCEL_ID: WidgetId = WidgetId(6011);

const WIN_W: f32 = 262.0;
const TITLE_H: f32 = 17.0;
const ART_Y: f32 = TITLE_H + 1.0;
const ART_H: f32 = 301.0;
const FOOTER_H: f32 = 30.0;
const WIN_H: f32 = ART_Y + ART_H + FOOTER_H;

// Positions inside the forge art
const HEX_X0: f32 = 10.0;
const HEX_STEP: f32 = 49.0;
const HEX_Y: f32 = 38.0;
const HEX_W: f32 = 46.0;
const HEX_H: f32 = 53.0;
const BLESSING_X: f32 = 206.0;
// Icons centred in the forge's two hexagons beside the spirit
const CHOSEN_MATERIAL: (f32, f32) = (79.0, 132.0);
const CHOSEN_BLESSING: (f32, f32) = (160.0, 132.0);
const ITEM_SLOT: Rect = Rect {
    x: 106.0,
    y: 185.0,
    w: 50.0,
    h: 45.0,
};
const ITEM_NAME_Y: f32 = 240.0;
const REFINE_BTN: Rect = Rect {
    x: 83.0,
    y: 245.0,
    w: 96.0,
    h: 48.0,
};
const BACK_BTN: Rect = Rect {
    x: 19.0,
    y: 250.0,
    w: 101.0,
    h: 38.0,
};
const AGAIN_BTN: Rect = Rect {
    x: 141.0,
    y: 250.0,
    w: 101.0,
    h: 50.0,
};
const ICON: f32 = 24.0;

// Frame timing: the empty forge loops and an item's panel slides in once at 250 ms a frame; the hammer and the
// result play once at 100 ms, then the result loops at 75 ms
const IDLE_MS: u64 = 250;
const PLAY_MS: u64 = 100;
const RESULT_LOOP_MS: u64 = 75;

const WAIT: [&str; 4] = [art::WAIT_0, art::WAIT_1, art::WAIT_2, art::WAIT_3];
const READY_A: [&str; 3] = [art::READY_A_0, art::READY_A_1, art::READY_A_2];
const READY_B: [&str; 3] = [art::READY_B_0, art::READY_B_1, art::READY_B_2];
const PROCESS_A: [&str; 14] = [
    art::PROCESS_A_0,
    art::PROCESS_A_1,
    art::PROCESS_A_2,
    art::PROCESS_A_3,
    art::PROCESS_A_4,
    art::PROCESS_A_5,
    art::PROCESS_A_6,
    art::PROCESS_A_7,
    art::PROCESS_A_8,
    art::PROCESS_9,
    art::PROCESS_10,
    art::PROCESS_11,
    art::PROCESS_12,
    art::PROCESS_13,
];
const PROCESS_B: [&str; 14] = [
    art::PROCESS_B_0,
    art::PROCESS_B_1,
    art::PROCESS_B_2,
    art::PROCESS_B_3,
    art::PROCESS_B_4,
    art::PROCESS_B_5,
    art::PROCESS_B_6,
    art::PROCESS_B_7,
    art::PROCESS_B_8,
    art::PROCESS_9,
    art::PROCESS_10,
    art::PROCESS_11,
    art::PROCESS_12,
    art::PROCESS_13,
];
const SUCCESS: [&str; 9] = [
    art::SUCCESS_0,
    art::SUCCESS_1,
    art::SUCCESS_2,
    art::SUCCESS_3,
    art::SUCCESS_4,
    art::SUCCESS_5,
    art::SUCCESS_6,
    art::SUCCESS_7,
    art::SUCCESS_8,
];
const SUCCESS_WAIT: [&str; 8] = [
    art::SUCCESS_9,
    art::SUCCESS_10,
    art::SUCCESS_11,
    art::SUCCESS_12,
    art::SUCCESS_13,
    art::SUCCESS_14,
    art::SUCCESS_15,
    art::SUCCESS_16,
];
const FAIL: [&str; 15] = [
    art::FAIL_0,
    art::FAIL_1,
    art::FAIL_2,
    art::FAIL_3,
    art::FAIL_4,
    art::FAIL_5,
    art::FAIL_6,
    art::FAIL_7,
    art::FAIL_8,
    art::FAIL_9,
    art::FAIL_10,
    art::FAIL_11,
    art::FAIL_12,
    art::FAIL_13,
    art::FAIL_14,
];
const FAIL_WAIT: [&str; 5] = [
    art::FAIL_15,
    art::FAIL_16,
    art::FAIL_17,
    art::FAIL_18,
    art::FAIL_19,
];
const SLOT_BLUE: [&str; MATERIAL_SLOTS] = [
    art::SLOT_BLUE_0,
    art::SLOT_BLUE_1,
    art::SLOT_BLUE_2,
    art::SLOT_BLUE_3,
];

const REFINE_TEX: ButtonTextures = ButtonTextures {
    normal: art::BT_REFINE_NORMAL,
    hover: art::BT_REFINE_NORMAL,
    pressed: art::BT_REFINE_PRESS,
};
const BACK_TEX: [ButtonTextures; 2] = [
    ButtonTextures {
        normal: art::BT_RETURN_SUCCESS_NORMAL,
        hover: art::BT_RETURN_SUCCESS_NORMAL,
        pressed: art::BT_RETURN_SUCCESS_PRESS,
    },
    ButtonTextures {
        normal: art::BT_RETURN_FAIL_NORMAL,
        hover: art::BT_RETURN_FAIL_NORMAL,
        pressed: art::BT_RETURN_FAIL_PRESS,
    },
];
const AGAIN_TEX: [ButtonTextures; 2] = [
    ButtonTextures {
        normal: art::BT_AGAIN_SUCCESS_NORMAL,
        hover: art::BT_AGAIN_SUCCESS_NORMAL,
        pressed: art::BT_AGAIN_SUCCESS_PRESS,
    },
    ButtonTextures {
        normal: art::BT_AGAIN_FAIL_NORMAL,
        hover: art::BT_AGAIN_FAIL_NORMAL,
        pressed: art::BT_AGAIN_FAIL_PRESS,
    },
];
const AGAIN_DISABLED: [&str; 2] = [art::BT_AGAIN_SUCCESS_DISABLE, art::BT_AGAIN_FAIL_DISABLE];
const CANCEL_TEX: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CANCEL,
    hover: nostalre_resources::ui::BTN_CANCEL_A,
    pressed: nostalre_resources::ui::BTN_CANCEL_B,
};

const RATE_COLOR: [f32; 4] = [
    0x29 as f32 / 255.0,
    0x63 as f32 / 255.0,
    0xde as f32 / 255.0,
    1.0,
];
const NAME_COLOR: [f32; 4] = [
    0x10 as f32 / 255.0,
    0x21 as f32 / 255.0,
    0x42 as f32 / 255.0,
    1.0,
];
const BUTTON_TEXT: [f32; 4] = [
    0x10 as f32 / 255.0,
    0x18 as f32 / 255.0,
    0x39 as f32 / 255.0,
    1.0,
];
const WHITE: [f32; 4] = [1.0; 4];
const DISABLED_TEXT: [f32; 4] = [0.55, 0.6, 0.72, 1.0];
const NOTICE_RED: [f32; 4] = [0.85, 0.1, 0.1, 1.0];
const NOTICE_BLUE: [f32; 4] = [0.1, 0.2, 0.85, 1.0];
const NOTICE_MS: u64 = 3000;

// msgstringtable lines
const MSG_TITLE: u16 = 3241;
const MSG_SUCCESS_RATE: u16 = 3724;
const MSG_UPGRADE_SUCCESS: u16 = 2971;
const MSG_UPGRADE_FAIL: u16 = 2972;
const MSG_RATE: u16 = 3285;
const MSG_BACK: u16 = 2973;
const MSG_USE_BLESSING: u16 = 2967;
const MSG_NO_ZENY: u16 = 2968;
const MSG_NO_BLESSING: u16 = 2969;
const MSG_NOT_REFINABLE: u16 = 2970;
const MSG_NO_MATERIAL: u16 = 3243;
const MSG_DESTROYED: u16 = 3246;

/// What the forge is doing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Empty, or an item in and waiting for the choice
    Ready,
    /// The hammer, until the server answers
    Hammer { since: u64, guarded: bool },
    /// The result: the hammer plays out, then the result's frames from `from` (never before the hammer ends);
    /// `broke`: the item is gone
    Result {
        hammer: u64,
        from: u64,
        guarded: bool,
        success: bool,
        broke: bool,
    },
}

pub struct RefineWindow {
    pub has_grf_textures: bool,
    phase: Phase,
    notice: Option<(u16, bool, u64)>,
    /// The item the panel last slid in for, and when
    ready_for: Option<u16>,
    ready_since: u64,
}

impl Default for RefineWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl RefineWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            phase: Phase::Ready,
            notice: None,
            ready_for: None,
            ready_since: 0,
        }
    }

    fn close(&mut self, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        ctx.character.refine.close();
        self.phase = Phase::Ready;
        self.notice = None;
        vec![GameEvent::RequestRefineClose]
    }

    fn notify(&mut self, msg_id: u16, error: bool, now: u64) {
        self.notice = Some((msg_id, error, now));
    }

    /// Asks for the refine the choice makes, or says what is missing
    fn refine(&mut self, ctx: &mut BuildCtx, now: u64, events: &mut Vec<GameEvent>) {
        let session = &mut ctx.character.refine;
        match session.request(&ctx.character.inventory) {
            Ok(request) => {
                session.start();
                self.phase = Phase::Hammer {
                    since: now,
                    guarded: session.blessings() > 0,
                };
                self.notice = None;
                events.push(GameEvent::RequestRefine {
                    index: request.index,
                    material: request.material,
                    blessing: request.blessing,
                });
            }
            Err(RefineBlock::NoMaterialChosen) => {}
            Err(RefineBlock::NoMaterial) => self.notify(MSG_NO_MATERIAL, true, now),
            Err(RefineBlock::NoZeny) => self.notify(MSG_NO_ZENY, true, now),
            Err(RefineBlock::NoBlessing) => self.notify(MSG_NO_BLESSING, true, now),
        }
    }

    /// The frame the forge shows now
    fn frame(&self, ctx: &BuildCtx, now: u64) -> &'static str {
        let session = &ctx.character.refine;
        match self.phase {
            Phase::Ready if session.item().is_none() => looped(&WAIT, now, IDLE_MS),
            Phase::Ready => {
                let panel = if session.blessings() > 0 {
                    &READY_A
                } else {
                    &READY_B
                };
                held(panel, now - self.ready_since, IDLE_MS)
            }
            Phase::Hammer { since, guarded } => held(hammer_frames(guarded), now - since, PLAY_MS),
            Phase::Result {
                hammer,
                from,
                guarded,
                success,
                ..
            } => {
                let (play, wait) = if success {
                    (&SUCCESS[..], &SUCCESS_WAIT[..])
                } else {
                    (&FAIL[..], &FAIL_WAIT[..])
                };
                if now < from {
                    held(hammer_frames(guarded), now - hammer, PLAY_MS)
                } else if now - from < play.len() as u64 * PLAY_MS {
                    held(play, now - from, PLAY_MS)
                } else {
                    looped(wait, now, RESULT_LOOP_MS)
                }
            }
        }
    }

    /// Whether the result's frames have played out, so its buttons show
    fn result_shown(&self, now: u64) -> Option<(bool, bool)> {
        match self.phase {
            Phase::Result {
                from,
                success,
                broke,
                ..
            } => {
                let length = if success { SUCCESS.len() } else { FAIL.len() } as u64 * PLAY_MS;
                (now >= from + length).then_some((success, broke))
            }
            _ => None,
        }
    }
}

fn hammer_frames(guarded: bool) -> &'static [&'static str] {
    if guarded { &PROCESS_A } else { &PROCESS_B }
}

/// A looping animation's frame at `now`, `ms` a frame
fn looped(frames: &[&'static str], now: u64, ms: u64) -> &'static str {
    frames[(now / ms) as usize % frames.len()]
}

/// A once-played animation's frame `played` ms in, `ms` a frame, its last frame held
fn held(frames: &[&'static str], played: u64, ms: u64) -> &'static str {
    frames[((played / ms) as usize).min(frames.len() - 1)]
}

fn draw_icon(ui: &mut UiFrame, ctx: &BuildCtx, item_id: u32, x: f32, y: f32) {
    if let Some(path) = ctx.data.item_icon_path(item_id) {
        draw_textured_quad(ui, x, y, ICON, ICON, path);
    }
}

/// One hexagon of the row over the forge: the item's icon, lit when chosen, how many the player carries over how
/// many a refine takes, the name on hover. Whether it was clicked.
fn hex_slot(
    ui: &mut UiFrame,
    ctx: &BuildCtx,
    id: WidgetId,
    hex: Rect,
    item_id: u32,
    needed: u8,
    lit: Option<&'static str>,
) -> bool {
    let response = ui.interact(id, hex);
    if let Some(lit) = lit {
        draw_textured_quad(ui, hex.x, hex.y, hex.w, hex.h, lit);
    }
    draw_icon(ui, ctx, item_id, hex.x + 11.0, hex.y + 14.0);
    let carried = ctx.character.inventory.count_of(item_id);
    ui.text_centered(
        hex.x - 2.0,
        hex.y + HEX_H + 12.0,
        50.0,
        &format!("{carried}/{needed}"),
        NAME_COLOR,
    );
    if response.hovered() {
        ui.any_interactive_hovered = true;
        ui.tooltip(
            hex.x + HEX_W / 2.0,
            hex.y,
            &ctx.data.item_name_for(item_id, true),
        );
    }
    response.clicked()
}

impl Window for RefineWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = art::ALL.to_vec();
        paths.extend([
            crate::helper::window_chrome::TITLEBAR_TEX,
            crate::helper::window_chrome::FOOTER_TEX,
            CANCEL_TEX.normal,
            CANCEL_TEX.hover,
            CANCEL_TEX.pressed,
        ]);
        paths
    }
}

impl InGameWindow for RefineWindow {
    fn wants_escape(&self, ctx: &BuildCtx) -> bool {
        ctx.character.refine.is_open()
    }

    fn on_escape(&mut self, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.close(ctx)
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !ctx.character.refine.is_open() {
            self.phase = Phase::Ready;
            return Vec::new();
        }
        let now = ui.ctx.now_ms;
        let mut events = Vec::new();
        if ctx.character.refine.item() != self.ready_for {
            self.ready_for = ctx.character.refine.item();
            self.ready_since = now;
        }
        if let Some(result) = ctx.character.refine.take_result()
            && let Phase::Hammer { since, guarded } = self.phase
        {
            // rAthena's ITEMREFINING_*: 0 success, 1 broke, 2 downgraded, 3 failed with nothing lost
            self.phase = Phase::Result {
                hammer: since,
                from: now.max(since + PROCESS_A.len() as u64 * PLAY_MS),
                guarded,
                success: result == 0,
                broke: result == 1,
            };
        }

        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;
        let win = ui.window_at(REFINE_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 330.0, 60.0);
        let (ox, oy) = (win.x, win.y);
        ui.interact(REFINE_WINDOW_ID, Rect::new(ox, oy, WIN_W, WIN_H));
        draw_titlebar(ui, ox, oy, WIN_W, TITLE_H, grf);
        let title = ctx.data.msg_or(MSG_TITLE, &[], "Refine");
        ui.text(ox + 18.0, oy + TITLE_H - 4.0, &title, text_color(grf));

        let (ax, ay) = (ox, oy + ART_Y);
        draw_textured_quad(ui, ax, ay, WIN_W, ART_H, self.frame(ctx, now));
        let at = |r: Rect| Rect::new(ax + r.x, ay + r.y, r.w, r.h);

        let item = ctx
            .character
            .refine
            .item()
            .and_then(|index| ctx.character.inventory.get_item(index))
            .cloned();
        let ready = self.phase == Phase::Ready;

        // Over the forge: the chosen material's rate while choosing, nothing under the hammer, then the result
        let shown = self.result_shown(now);
        let plate = match (self.phase, shown) {
            (Phase::Ready, _) => {
                let chance = ctx.character.refine.selected().map_or(0, |m| m.chance);
                Some(
                    ctx.data
                        .msg_or(MSG_SUCCESS_RATE, &[&chance.to_string()], "Success %"),
                )
            }
            (_, Some((true, _))) => Some(ctx.data.msg_or(
                MSG_UPGRADE_SUCCESS,
                &[],
                "Upgrade success!",
            )),
            (_, Some((false, _))) => {
                Some(ctx.data.msg_or(MSG_UPGRADE_FAIL, &[], "Upgrade failed!"))
            }
            _ => None,
        };
        if let Some(plate) = plate {
            ui.text_centered(ax, ay + 26.0, WIN_W, &plate, RATE_COLOR);
        }

        // The materials offered and the Blacksmith Blessing, while choosing
        if ready && item.is_some() {
            let materials = ctx.character.refine.materials().to_vec();
            for (slot, material) in materials.iter().enumerate() {
                let hex = Rect::new(
                    ax + HEX_X0 + slot as f32 * HEX_STEP,
                    ay + HEX_Y,
                    HEX_W,
                    HEX_H,
                );
                let lit =
                    (ctx.character.refine.selected_slot() == Some(slot)).then_some(SLOT_BLUE[slot]);
                let id = WidgetId(MATERIAL_BASE + slot as u32);
                if hex_slot(ui, ctx, id, hex, material.item_id, 1, lit) {
                    ctx.character.refine.select(slot);
                }
            }
            let blessings = ctx.character.refine.blessings();
            if blessings > 0 {
                let hex = Rect::new(ax + BLESSING_X, ay + HEX_Y, HEX_W, HEX_H);
                let lit = ctx.character.refine.use_blessing().then_some(art::SLOT_RED);
                if hex_slot(
                    ui,
                    ctx,
                    BLESSING_ID,
                    hex,
                    BLACKSMITH_BLESSING,
                    blessings,
                    lit,
                ) {
                    ctx.character.refine.toggle_blessing();
                    if ctx.character.refine.use_blessing() {
                        self.notify(MSG_USE_BLESSING, false, now);
                    }
                }
            }
            if materials.is_empty() {
                self.notice.get_or_insert((MSG_NOT_REFINABLE, true, now));
            }
            // The choice beside the spirit
            if let Some(material) = ctx.character.refine.selected().copied() {
                draw_icon(
                    ui,
                    ctx,
                    material.item_id,
                    ax + CHOSEN_MATERIAL.0,
                    ay + CHOSEN_MATERIAL.1,
                );
            }
            if ctx.character.refine.use_blessing() {
                draw_icon(
                    ui,
                    ctx,
                    BLACKSMITH_BLESSING,
                    ax + CHOSEN_BLESSING.0,
                    ay + CHOSEN_BLESSING.1,
                );
            }
        }

        // The item slot: an equipment goes in by drag from the inventory (double-click there too), and comes out
        // by double-click or a drag out of the slot
        let slot = at(ITEM_SLOT);
        let response = ui.interact(ITEM_SLOT_ID, slot);
        if let Some(icon) = item.as_ref().and_then(|item| item.icon_path()) {
            draw_textured_quad(ui, slot.x + 13.0, slot.y + 7.0, ICON, ICON, icon);
        }
        if ready && let Some(item) = item.as_ref() {
            if response.double_clicked() {
                ctx.character.refine.clear_item();
            } else if response.clicked() {
                ui.drag_source(
                    REFINE_WINDOW_ID,
                    item.index as usize,
                    item.icon_path(),
                    (ICON, ICON),
                );
            }
        }
        // The name shows the new level only once the result has played (the server's answer comes first)
        if let Some(item) = item.as_ref().filter(|_| ready || shown.is_some()) {
            let name = if item.refining_level > 0 {
                format!("+{} {}", item.refining_level, item.name)
            } else {
                item.name.clone()
            };
            ui.text_centered(ax, ay + ITEM_NAME_Y, WIN_W, &name, NAME_COLOR);
        }
        if let Some((_, true)) = shown {
            let destroyed = ctx
                .data
                .msg_or(MSG_DESTROYED, &[], "This item has been destroyed.");
            ui.text_centered(ax, ay + ITEM_NAME_Y, WIN_W, &destroyed, NAME_COLOR);
        }
        if ready
            && let Some((source, index)) = ui.drop_zone(slot)
            && source == INV_WINDOW_ID
        {
            let index = index as u16;
            if ctx
                .character
                .inventory
                .get_item(index)
                .is_some_and(refinable)
            {
                events.push(GameEvent::RequestRefineSelect { index });
            } else {
                ctx.character.refine.refuse();
            }
        }
        if ctx.character.refine.take_refused() {
            self.notify(MSG_NOT_REFINABLE, true, now);
        }

        // Refine, with the zeny it costs; greyed until a material is chosen (the empty forge's art has the button)
        if ready {
            let button = at(REFINE_BTN);
            let zeny = ctx.character.refine.selected().map(|m| m.zeny);
            if zeny.is_some() {
                if ui
                    .button(REFINE_ID, button, &REFINE_TEX, "Refine")
                    .clicked()
                {
                    self.refine(ctx, now, &mut events);
                }
            } else if grf && item.is_some() {
                draw_textured_quad(
                    ui,
                    button.x,
                    button.y,
                    button.w,
                    button.h,
                    art::BT_REFINE_DISABLE,
                );
            }
            let color = if zeny.is_some() {
                BUTTON_TEXT
            } else {
                DISABLED_TEXT
            };
            ui.text_centered(button.x, button.y + 19.0, button.w, &title, color);
            if let Some(zeny) = zeny {
                ui.text_right(button.x + 87.0, button.y + 39.0, &zeny.to_string(), WHITE);
            }
        }

        // After the result: Back, and Refine again with the same choice
        if let Some((success, broke)) = shown {
            let style = usize::from(!success);
            let back = at(BACK_BTN);
            let label = ctx.data.msg_or(MSG_BACK, &[], crate::pt!("Back", "Voltar"));
            if ui.button(BACK_ID, back, &BACK_TEX[style], &label).clicked() {
                if broke {
                    ctx.character.refine.clear_item();
                }
                self.phase = Phase::Ready;
                self.ready_since = now;
            }
            ui.text_centered(back.x, back.y + 24.0, back.w, &label, BUTTON_TEXT);
            let again = at(AGAIN_BTN);
            let can_again = !broke
                && ctx
                    .character
                    .refine
                    .request(&ctx.character.inventory)
                    .is_ok();
            if can_again {
                if ui
                    .button(AGAIN_ID, again, &AGAIN_TEX[style], &title)
                    .clicked()
                {
                    self.refine(ctx, now, &mut events);
                }
            } else if grf {
                draw_textured_quad(
                    ui,
                    again.x,
                    again.y,
                    again.w,
                    again.h,
                    AGAIN_DISABLED[style],
                );
            }
            if let Some(material) = ctx.character.refine.selected().copied().filter(|_| !broke) {
                ui.text_centered(again.x, again.y + 16.0, again.w, &title, BUTTON_TEXT);
                let rate = ctx
                    .data
                    .msg_or(MSG_RATE, &[&material.chance.to_string()], "Rate");
                ui.text_centered(again.x, again.y + 29.0, again.w, &rate, BUTTON_TEXT);
                ui.text_centered(
                    again.x,
                    again.y + 44.0,
                    again.w,
                    &material.zeny.to_string(),
                    WHITE,
                );
            }
        }

        // The footer: a notice, and Cancel
        let footer_y = oy + ART_Y + ART_H;
        draw_footer(ui, ox, footer_y, WIN_W, FOOTER_H, grf);
        if let Some((msg_id, error, since)) = self.notice {
            if !error || now - since < NOTICE_MS {
                if grf {
                    draw_textured_quad(ui, ox + 5.0, footer_y + 8.0, 14.0, 13.0, art::ICO_NOTICE);
                }
                let text = ctx.data.msg_or(msg_id, &[], "");
                let color = if error { NOTICE_RED } else { NOTICE_BLUE };
                ui.text(ox + 22.0, footer_y + 19.0, &text, color);
            } else {
                self.notice = None;
            }
        }
        let cancel = Rect::new(ox + WIN_W - 47.0, footer_y + 5.0, 42.0, 20.0);
        // Cancel closes even under the hammer: the server drops a refine it refuses without an answer
        if ui
            .button(CANCEL_ID, cancel, &CANCEL_TEX, crate::pt!("Cancel", "Cancelar"))
            .clicked()
        {
            events.extend(self.close(ctx));
        }

        ui.has_grf_textures = prev_grf;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loops_wrap_and_played_animations_hold_their_last_frame() {
        assert_eq!(looped(&WAIT, 0, IDLE_MS), art::WAIT_0);
        assert_eq!(looped(&WAIT, 4 * IDLE_MS, IDLE_MS), art::WAIT_0);
        assert_eq!(looped(&WAIT, 5 * IDLE_MS + 1, IDLE_MS), art::WAIT_1);
        assert_eq!(held(&READY_A, 0, IDLE_MS), art::READY_A_0);
        assert_eq!(held(&READY_A, 60 * IDLE_MS, IDLE_MS), art::READY_A_2);
        assert_eq!(held(&SUCCESS, 100 * PLAY_MS, PLAY_MS), art::SUCCESS_8);
    }
}
