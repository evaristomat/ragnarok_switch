//! The Renewal basic info window (kRO 2020 and later): 220 wide, `basewin_bg2`, or `w_basewin_bg2` with an AP row
//! for 4th jobs, and under it the menu: a strip that opens and closes a panel of `menu_icon` buttons.

use crate::helper::window_chrome::{
    draw_gauge, draw_sys_button, draw_textured_quad, push_quad, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::character::{Character, is_fourth_job, job_class_name};
use nostalre_game::event::GameEvent;
use nostalre_game::keybinding::HotkeyAction;
use nostalre_resources::ui::{basic, menu_icon};
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const BASIC_INFO_WINDOW_ID: WidgetId = WidgetId(1400);
const MINI_BTN_ID: WidgetId = WidgetId(1401);
const CLOSE_BTN_ID: WidgetId = WidgetId(1402);
const MENU_TOGGLE_ID: WidgetId = WidgetId(1403);
/// The menu's buttons, 1410 on
const MENU_BTN_FIRST_ID: u32 = 1410;

const BG_TEX: &str = basic::BASEWIN_BG2;
const BG_AP_TEX: &str = basic::W_BASEWIN_BG2;
const AP_TRACK_TEX: &str = basic::GZE_BG1;

const WIN_W: f32 = 220.0;
const TITLE_H: f32 = 17.0;
/// The window without and with the AP row, and minimized
const WIN_H: f32 = 135.0;
const AP_ROW_H: f32 = 15.0;
const WIN_H_SMALL: f32 = 70.0;

/// HP, SP and AP gauges: the tracks of HP and SP are in the background, AP's is `gze_bg1`
const BAR_X: f32 = 35.0;
const HP_BAR_Y: f32 = 53.0;
const BAR_W: f32 = 135.0;
const BAR_H: f32 = 9.0;
const BAR_FILL_H: f32 = 8.0;
const BAR_CAP_W: f32 = 4.0;
const BAR_TITLE_X: f32 = 15.0;
const BAR_PCT_RIGHT: f32 = 200.0;
/// HP and SP turn red below a quarter
const BAR_LOW: f32 = 0.25;

/// Base and job level, their exp bars and the weight line, before the AP row moves them down
const EXP_BAR_X: f32 = 84.0;
const EXP_BAR_Y: f32 = 89.0;
const EXP_BAR_W: f32 = 110.0;
const EXP_BAR_H: f32 = 4.0;
/// The level text's baseline, below the top of its exp bar
const LEVEL_TEXT_DROP: f32 = 8.0;
const LINE_H: f32 = 12.0;
const WEIGHT_BASELINE: f32 = 130.0;
const WEIGHT_RIGHT: f32 = 215.0;

/// The menu strip under the window, and the panel of buttons under the strip
const MENU_STRIP_W: f32 = 219.0;
const MENU_STRIP_H: f32 = 9.0;
const MENU_PANEL_GAP: f32 = 1.0;
const MENU_COLUMNS: usize = 5;
const MENU_CELL: f32 = 44.0;
const MENU_BTN_INSET: f32 = 6.0;
const MENU_BTN_W: f32 = 32.0;
const MENU_BTN_H: f32 = 34.0;
const MENU_BG_SIZE: (f32, f32) = (220.0, 225.0);

const MSG_BASIC_INFO: u16 = 238;

/// What a menu button does: a window the hotkeys also open, the system menu, or the minimap's visibility
#[derive(Clone, Copy, Debug)]
enum MenuAction {
    Hotkey(HotkeyAction),
    SystemMenu,
    Minimap,
}

impl MenuAction {
    fn event(self) -> GameEvent {
        match self {
            MenuAction::Hotkey(action) => GameEvent::RunHotkeyAction { action },
            MenuAction::SystemMenu => GameEvent::ToggleSystemMenu,
            MenuAction::Minimap => GameEvent::ToggleMinimap,
        }
    }
}

struct MenuButton {
    action: MenuAction,
    /// What a right click does (also the controller's B): the minimap on the map button, as the old Map button
    right_click: Option<MenuAction>,
    normal: &'static str,
    pressed: &'static str,
    /// Without the hotkey: the hotkeys can be rebound
    tip: &'static str,
}

const fn hotkey(
    action: HotkeyAction,
    normal: &'static str,
    pressed: &'static str,
    tip: &'static str,
) -> MenuButton {
    MenuButton {
        action: MenuAction::Hotkey(action),
        right_click: None,
        normal,
        pressed,
        tip,
    }
}

/// The menu's buttons in kRO's order, those with a window in this client
const MENU: [MenuButton; 9] = [
    hotkey(
        HotkeyAction::ToggleStatus,
        menu_icon::BT_STATUS,
        menu_icon::BT_STATUS_PRESS,
        "Status",
    ),
    hotkey(
        HotkeyAction::ToggleEquipment,
        menu_icon::BT_EQUIP,
        menu_icon::BT_EQUIP_PRESS,
        crate::pt!("Equipment", "Equipamentos"),
    ),
    hotkey(
        HotkeyAction::ToggleInventory,
        menu_icon::BT_ITEM,
        menu_icon::BT_ITEM_PRESS,
        crate::pt!("Inventory", "Inventário"),
    ),
    hotkey(
        HotkeyAction::ToggleSkillTree,
        menu_icon::BT_SKILL,
        menu_icon::BT_SKILL_PRESS,
        crate::pt!("Skills", "Habilidades"),
    ),
    hotkey(
        HotkeyAction::ToggleParty,
        menu_icon::BT_PARTY,
        menu_icon::BT_PARTY_PRESS,
        crate::pt!("Party", "Grupo"),
    ),
    hotkey(
        HotkeyAction::ToggleGuild,
        menu_icon::BT_GUILD,
        menu_icon::BT_GUILD_PRESS,
        crate::pt!("Guild", "Clã"),
    ),
    hotkey(
        HotkeyAction::ToggleQuest,
        menu_icon::BT_QUEST,
        menu_icon::BT_QUEST_PRESS,
        "Quest",
    ),
    MenuButton {
        action: MenuAction::Hotkey(HotkeyAction::ToggleWorldMap),
        right_click: Some(MenuAction::Minimap),
        normal: menu_icon::BT_MAP,
        pressed: menu_icon::BT_MAP_PRESS,
        tip: "World Map (right click: minimap)",
    },
    MenuButton {
        action: MenuAction::SystemMenu,
        right_click: None,
        normal: menu_icon::BT_OPTION,
        pressed: menu_icon::BT_OPTION_PRESS,
        tip: "Option",
    },
];

/// A gauge row: its title, values, fill colour and, when the background has none, its track
struct Gauge<'a> {
    title: &'a str,
    cur: u32,
    max: u32,
    red: bool,
    track: Option<&'static str>,
}

impl Gauge<'_> {
    fn ratio(&self) -> f32 {
        if self.max > 0 {
            self.cur as f32 / self.max as f32
        } else {
            0.0
        }
    }
}

pub struct BasicInfoWindow {
    pub has_grf_textures: bool,
    hidden: bool,
    minimized: bool,
    menu_open: bool,
    sys_btn_size: (f32, f32),
}

impl Default for BasicInfoWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicInfoWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            hidden: false,
            minimized: false,
            menu_open: true,
            sys_btn_size: (11.0, 11.0),
        }
    }

    pub fn toggle(&mut self) {
        self.hidden = !self.hidden;
    }

    pub fn is_menu_open(&self) -> bool {
        self.menu_open
    }

    /// The window's own height (without the menu) for a job: 4th jobs get the AP row
    fn body_height(&self, job_class: u16) -> f32 {
        if self.minimized {
            WIN_H_SMALL
        } else if is_fourth_job(job_class) {
            WIN_H + AP_ROW_H
        } else {
            WIN_H
        }
    }

    fn menu_rows() -> usize {
        MENU.len().div_ceil(MENU_COLUMNS)
    }

    fn menu_height(&self) -> f32 {
        let panel = if self.menu_open {
            MENU_PANEL_GAP + Self::menu_rows() as f32 * MENU_CELL
        } else {
            0.0
        };
        MENU_STRIP_H + panel
    }

    fn display_name(character: &Character) -> &str {
        if character.name.is_empty() {
            "Unknown"
        } else {
            &character.name
        }
    }

    /// The top (or bottom) `h` rows of `tex`, `tex_h` tall, at the window's width
    fn draw_texture_rows(
        ui: &mut UiFrame,
        x: f32,
        y: f32,
        h: f32,
        tex: &'static str,
        tex_h: f32,
        from_bottom: bool,
    ) {
        let (v0, v1) = if from_bottom {
            (1.0 - h / tex_h, 1.0)
        } else {
            (0.0, h / tex_h)
        };
        let (v, i) = draw::quad_vertices_uv(x, y, WIN_W, h, [0.0, v0], [1.0, v1], [1.0; 4]);
        ui.draw_calls.push(DrawCall {
            vertices: v.into(),
            indices: i.into(),
            texture: TextureRef::Named(tex.into()),
        });
    }

    /// The titlebar's two buttons, the same in both modes: the base button (no action) and the one that
    /// minimizes or restores the window
    fn title_buttons(&mut self, ui: &mut UiFrame, x: f32, y: f32) {
        let (w, h) = self.sys_btn_size;
        let glyph = if self.minimized { '+' } else { '_' };
        let buttons = [
            (
                CLOSE_BTN_ID,
                Rect::new(x + 4.0, y + 3.0, w, h),
                basic::SYS_BASE_ON,
                basic::SYS_BASE_OFF,
                None,
            ),
            (
                MINI_BTN_ID,
                Rect::new(x + WIN_W - 2.0 - w, y + 3.0, w, h),
                basic::SYS_MINI_ON,
                basic::SYS_MINI_OFF,
                Some(glyph),
            ),
        ];
        for (id, rect, on, off, glyph) in buttons {
            let resp = ui.interact(id, rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            draw_sys_button(
                ui,
                rect,
                (w, h),
                resp.hovered(),
                self.has_grf_textures,
                on,
                off,
                glyph,
            );
            if id == MINI_BTN_ID && resp.clicked() {
                self.minimized = !self.minimized;
            }
        }
    }

    /// One gauge row from `y`: title, track, fill, "cur / max" in the bar and the percentage
    fn gauge_row(ui: &mut UiFrame, x: f32, y: f32, gauge: Gauge, grf: bool) {
        let tc = text_color(grf);
        let baseline = y + BAR_H - 1.0;
        ui.text(x + BAR_TITLE_X, baseline, gauge.title, tc);
        if let (true, Some(track)) = (grf, gauge.track) {
            draw_textured_quad(ui, x + BAR_X, y, BAR_W, BAR_H, track);
        }
        let ratio = gauge.ratio();
        draw_gauge(
            ui,
            x + BAR_X,
            y,
            BAR_W,
            BAR_FILL_H,
            BAR_CAP_W,
            ratio,
            gauge.red,
            grf,
        );
        let values = format!("{} / {}", gauge.cur, gauge.max);
        ui.text_centered(x + BAR_X, baseline, BAR_W, &values, tc);
        let shown = (ratio * 100.0).floor().clamp(0.0, 100.0) as u32;
        ui.text_right(x + BAR_PCT_RIGHT, baseline, &format!("{shown}%"), tc);
    }

    /// HP or SP: red below a quarter
    fn pool(title: &str, cur: u32, max: u32) -> Gauge<'_> {
        let mut gauge = Gauge {
            title,
            cur,
            max,
            red: false,
            track: None,
        };
        gauge.red = gauge.max > 0 && gauge.ratio() < BAR_LOW;
        gauge
    }

    fn exp_row(ui: &mut UiFrame, x: f32, y: f32, label: &str, pct: f32, grf: bool) {
        ui.text(x + BAR_TITLE_X, y + LEVEL_TEXT_DROP, label, text_color(grf));
        crate::helper::window_chrome::draw_exp_bar(
            ui,
            x + EXP_BAR_X,
            y,
            EXP_BAR_W,
            EXP_BAR_H,
            pct,
            grf,
        );
        let rect = Rect::new(x + EXP_BAR_X, y, EXP_BAR_W + 2.0, EXP_BAR_H + 2.0);
        if rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
            let text = format!("{:.1}%", (pct * 1000.0).floor() * 0.1);
            ui.tooltip(ui.ctx.mouse_x + 10.0, ui.ctx.mouse_y, &text);
        }
    }

    fn build_large(&mut self, ui: &mut UiFrame, ctx: &BuildCtx, x: f32, y: f32) {
        let grf = self.has_grf_textures;
        let tc = text_color(grf);
        let character = &*ctx.character;
        let fourth = is_fourth_job(ctx.job_class);
        let h = self.body_height(ctx.job_class);
        if grf {
            draw_textured_quad(ui, x, y, WIN_W, h, if fourth { BG_AP_TEX } else { BG_TEX });
        } else {
            crate::helper::fallback::panel(ui, x, y, WIN_W, h);
        }
        self.title_buttons(ui, x, y);
        let title = ctx
            .data
            .msg_string
            .as_ref()
            .and_then(|t| t.get(MSG_BASIC_INFO))
            .unwrap_or("Basic Info");
        ui.text(x + 18.0, y + 13.0, title, tc);
        ui.text(x + 10.0, y + 31.0, Self::display_name(character), tc);
        ui.text(x + 10.0, y + 44.0, job_class_name(ctx.job_class), tc);

        let hp = Self::pool("HP", character.hp, character.max_hp);
        Self::gauge_row(ui, x, y + HP_BAR_Y, hp, grf);
        let sp = Self::pool("SP", character.sp, character.max_sp);
        Self::gauge_row(ui, x, y + HP_BAR_Y + AP_ROW_H, sp, grf);
        let shift = if fourth {
            let (ap, max_ap) = (character.traits.ap, character.traits.max_ap);
            let ap = Gauge {
                title: "AP",
                cur: ap,
                max: max_ap,
                // AP turns red when full: it is spent, not lost
                red: max_ap > 0 && ap >= max_ap,
                track: Some(AP_TRACK_TEX),
            };
            Self::gauge_row(ui, x, y + HP_BAR_Y + 2.0 * AP_ROW_H, ap, grf);
            AP_ROW_H
        } else {
            0.0
        };

        let exp_y = y + EXP_BAR_Y + shift;
        let base = format!("Base Lv. {}", character.base_level);
        Self::exp_row(ui, x, exp_y, &base, character.base_exp_percentage(), grf);
        let job = format!("Job Lv. {}", character.job_level);
        Self::exp_row(
            ui,
            x,
            exp_y + LINE_H,
            &job,
            character.job_exp_percentage(),
            grf,
        );

        let (weight, max_weight) = (character.inventory.weight, character.inventory.max_weight);
        let over = character.inventory.is_overweight();
        let weight_text = format!("Weight : {} / {}", weight / 10, max_weight / 10);
        let zeny_text = format!("Zeny : {}", format_zeny(character.inventory.zeny));
        let baseline = y + WEIGHT_BASELINE + shift;
        let zeny_w = ui.atlas.measure_text(&zeny_text);
        ui.text_right(x + WEIGHT_RIGHT, baseline, &zeny_text, tc);
        let weight_color = if over { crate::helper::colors::RED } else { tc };
        ui.text_right(
            x + WEIGHT_RIGHT - zeny_w - 8.0,
            baseline,
            &weight_text,
            weight_color,
        );
    }

    fn build_small(&mut self, ui: &mut UiFrame, ctx: &BuildCtx, x: f32, y: f32) {
        let grf = self.has_grf_textures;
        let tc = text_color(grf);
        let character = &*ctx.character;
        if grf {
            let body_top = y + TITLE_H + 1.0;
            let body_h = WIN_H_SMALL - TITLE_H - 1.0;
            Self::draw_texture_rows(ui, x, y, TITLE_H + 1.0, BG_TEX, WIN_H, false);
            push_quad(ui, x, body_top, WIN_W, body_h, [0.69, 0.69, 0.69, 1.0]);
            push_quad(ui, x + 1.0, body_top, WIN_W - 2.0, body_h - 1.0, [1.0; 4]);
        } else {
            crate::helper::fallback::panel(ui, x, y, WIN_W, WIN_H_SMALL);
        }
        self.title_buttons(ui, x, y);
        ui.text(x + 18.0, y + 13.0, Self::display_name(character), tc);
        let job = job_class_name(ctx.job_class);
        let line2 = format!(
            "Lv.{} / {} / Lv.{}",
            character.base_level, job, character.job_level
        );
        ui.text_bold(x + 10.0, y + 31.0, &line2, tc);
        ui.text(
            x + 10.0,
            y + 48.0,
            &format!("HP. {} / {}", character.hp, character.max_hp),
            tc,
        );
        let exp = (character.base_exp_percentage() * 1000.0).floor() * 0.1;
        ui.text(x + 150.0, y + 48.0, &format!("| Exp. {exp:.1}%"), tc);
        ui.text(
            x + 10.0,
            y + 63.0,
            &format!("SP. {} / {}", character.sp, character.max_sp),
            tc,
        );
        if is_fourth_job(ctx.job_class) {
            let ap = format!(
                "| AP. {} / {}",
                character.traits.ap, character.traits.max_ap
            );
            ui.text(x + 150.0, y + 63.0, &ap, tc);
        }
    }

    /// The strip under the window, and while it is open the panel of buttons
    fn build_menu(&mut self, ui: &mut UiFrame, x: f32, y: f32, events: &mut Vec<GameEvent>) {
        let grf = self.has_grf_textures;
        let strip = Rect::new(x, y, MENU_STRIP_W, MENU_STRIP_H);
        let resp = ui.interact(MENU_TOGGLE_ID, strip);
        if resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        if grf {
            let pressed = resp.hovered() && ui.ctx.mouse_down;
            let tex = match (self.menu_open, resp.hovered(), pressed) {
                (true, _, true) => menu_icon::BT_MENU_CLOSE_PRESS,
                (true, true, false) => menu_icon::BT_MENU_CLOSE_OVER,
                (true, false, _) => menu_icon::BT_MENU_CLOSE_NORMAL,
                (false, _, true) => menu_icon::BT_MENU_PRESS,
                (false, true, false) => menu_icon::BT_MENU_OVER,
                (false, false, _) => menu_icon::BT_MENU_NORMAL,
            };
            draw_textured_quad(ui, strip.x, strip.y, MENU_STRIP_W, MENU_STRIP_H, tex);
        } else {
            push_quad(
                ui,
                strip.x,
                strip.y,
                strip.w,
                strip.h,
                [0.75, 0.82, 0.95, 1.0],
            );
        }
        if resp.clicked() {
            self.menu_open = !self.menu_open;
        }
        if !self.menu_open {
            return;
        }

        let top = y + MENU_STRIP_H + MENU_PANEL_GAP;
        let panel_h = Self::menu_rows() as f32 * MENU_CELL;
        if grf {
            Self::draw_texture_rows(
                ui,
                x,
                top,
                panel_h,
                menu_icon::BG_MENU,
                MENU_BG_SIZE.1,
                true,
            );
        } else {
            push_quad(ui, x, top, WIN_W, panel_h, [0.12, 0.22, 0.33, 0.85]);
        }
        for (n, button) in MENU.iter().enumerate() {
            let cell_x = x + (n % MENU_COLUMNS) as f32 * MENU_CELL + MENU_BTN_INSET;
            let cell_y = top + (n / MENU_COLUMNS) as f32 * MENU_CELL + MENU_BTN_INSET;
            let rect = Rect::new(cell_x, cell_y, MENU_BTN_W, MENU_BTN_H);
            let resp = ui.interact(WidgetId(MENU_BTN_FIRST_ID + n as u32), rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
                ui.tooltip(ui.ctx.mouse_x + 10.0, ui.ctx.mouse_y, button.tip);
            }
            if grf {
                let pressed = resp.hovered() && ui.ctx.mouse_down;
                let tex = if pressed {
                    button.pressed
                } else {
                    button.normal
                };
                draw_textured_quad(ui, rect.x, rect.y, MENU_BTN_W, MENU_BTN_H, tex);
            } else {
                let c = if resp.hovered() {
                    [0.5, 0.6, 0.8, 1.0]
                } else {
                    [0.3, 0.4, 0.6, 1.0]
                };
                push_quad(ui, rect.x, rect.y, rect.w, rect.h, c);
            }
            if resp.clicked() {
                events.push(button.action.event());
            }
            if let (true, Some(action)) = (resp.right_clicked(), button.right_click) {
                events.push(action.event());
            }
        }
    }
}

impl Window for BasicInfoWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn set_texture_sizes(&mut self, size_fn: &dyn Fn(&str) -> Option<(u32, u32)>) {
        if let Some((w, h)) = size_fn(basic::SYS_MINI_OFF) {
            self.sys_btn_size = (w as f32, h as f32);
        }
    }

    fn window_size(&self) -> (f32, f32) {
        (WIN_W, self.body_height(0) + self.menu_height())
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = vec![
            BG_TEX,
            BG_AP_TEX,
            AP_TRACK_TEX,
            basic::SYS_BASE_OFF,
            basic::SYS_BASE_ON,
            basic::SYS_MINI_OFF,
            basic::SYS_MINI_ON,
            menu_icon::BG_MENU,
            menu_icon::BT_MENU_NORMAL,
            menu_icon::BT_MENU_OVER,
            menu_icon::BT_MENU_PRESS,
            menu_icon::BT_MENU_CLOSE_NORMAL,
            menu_icon::BT_MENU_CLOSE_OVER,
            menu_icon::BT_MENU_CLOSE_PRESS,
        ];
        paths.extend(crate::helper::window_chrome::gauge_texture_paths());
        paths.extend(MENU.iter().flat_map(|b| [b.normal, b.pressed]));
        paths
    }
}

impl InGameWindow for BasicInfoWindow {
    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if self.hidden {
            return Vec::new();
        }
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let body_h = self.body_height(ctx.job_class);
        let total_h = body_h + self.menu_height();
        let win = ui.window_at(BASIC_INFO_WINDOW_ID, WIN_W, total_h, TITLE_H, 0.0, 0.0);
        ui.interact(
            BASIC_INFO_WINDOW_ID,
            Rect::new(win.x, win.y, WIN_W, total_h),
        );

        let mut events = Vec::new();
        if self.minimized {
            self.build_small(ui, ctx, win.x, win.y);
        } else {
            self.build_large(ui, ctx, win.x, win.y);
        }
        self.build_menu(ui, win.x, win.y + body_h, &mut events);

        ui.has_grf_textures = prev_grf;
        events
    }
}

fn format_zeny(value: i32) -> String {
    if value < 0 {
        return format!("-{}", format_zeny(-value));
    }
    let s = value.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_game::data_table::DataTable;
    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    #[derive(Clone, Copy)]
    enum Click {
        Left(f32, f32),
        Right(f32, f32),
    }

    fn frame(
        win: &mut BasicInfoWindow,
        character: &mut Character,
        job: u16,
        click: Option<Click>,
    ) -> (Vec<GameEvent>, Vec<String>) {
        let data = DataTable::new();
        let mut ctx = UiContext::new(1024.0, 768.0);
        match click {
            Some(Click::Left(mx, my)) => {
                (ctx.mouse_x, ctx.mouse_y, ctx.mouse_clicked) = (mx, my, true);
            }
            Some(Click::Right(mx, my)) => {
                (ctx.mouse_x, ctx.mouse_y, ctx.mouse_right_clicked) = (mx, my, true);
            }
            None => {}
        }
        let mut state = StateCache::new();
        let mut ui = test_frame(&mut ctx, &mut state);
        ui.has_grf_textures = true;
        win.has_grf_textures = true;
        let mut build_ctx = crate::BuildCtx::test(character, &data);
        build_ctx.job_class = job;
        let events = win.build(&mut ui, &mut build_ctx);
        let textures = ui
            .draw_calls
            .iter()
            .filter_map(|c| match &c.texture {
                TextureRef::Named(name) => Some(name.to_string()),
                _ => None,
            })
            .collect();
        (events, textures)
    }

    /// The middle of menu button `n` under a window `body` tall
    fn cell(n: usize, body: f32) -> (f32, f32) {
        let top = body + MENU_STRIP_H + MENU_PANEL_GAP + MENU_BTN_INSET + 10.0;
        let col = (n % MENU_COLUMNS) as f32;
        let row = (n / MENU_COLUMNS) as f32;
        (
            MENU_BTN_INSET + col * MENU_CELL + 10.0,
            top + row * MENU_CELL,
        )
    }

    #[test]
    fn classic_characters_never_show_ap() {
        let mut character = Character::new();
        character.traits.ap = 40;
        character.traits.max_ap = 200;
        let (_, textures) = frame(&mut BasicInfoWindow::new(), &mut character, 4252, None);
        assert!(!textures.iter().any(|t| t == BG_AP_TEX));
        assert!(!textures.iter().any(|t| t == AP_TRACK_TEX));

        let (_, textures) = frame(&mut BasicInfoWindow::new(), &mut character, 4060, None);
        assert!(textures.iter().any(|t| t == BG_TEX));
        assert!(!textures.iter().any(|t| t == AP_TRACK_TEX || t == BG_AP_TEX));
    }

    #[test]
    fn the_strip_closes_and_opens_the_menu() {
        let mut win = BasicInfoWindow::new();
        let mut character = Character::new();
        let strip = Click::Left(100.0, WIN_H + 4.0);
        frame(&mut win, &mut character, 0, Some(strip));
        assert!(!win.is_menu_open());
        let (_, textures) = frame(&mut win, &mut character, 0, None);
        assert!(!textures.iter().any(|t| t == menu_icon::BG_MENU));
        frame(&mut win, &mut character, 0, Some(strip));
        assert!(win.is_menu_open());
    }

    #[test]
    fn menu_buttons_open_their_windows() {
        let mut character = Character::new();
        for (n, expected) in [
            (
                0,
                GameEvent::RunHotkeyAction {
                    action: HotkeyAction::ToggleStatus,
                },
            ),
            (
                5,
                GameEvent::RunHotkeyAction {
                    action: HotkeyAction::ToggleGuild,
                },
            ),
            (
                7,
                GameEvent::RunHotkeyAction {
                    action: HotkeyAction::ToggleWorldMap,
                },
            ),
            (8, GameEvent::ToggleSystemMenu),
        ] {
            let (x, y) = cell(n, WIN_H);
            let (events, _) = frame(
                &mut BasicInfoWindow::new(),
                &mut character,
                0,
                Some(Click::Left(x, y)),
            );
            assert_eq!(
                format!("{events:?}"),
                format!("{:?}", [expected]),
                "button {n}"
            );
        }
    }

    #[test]
    fn a_right_click_on_the_map_button_cycles_the_minimap() {
        let mut character = Character::new();
        let (x, y) = cell(7, WIN_H);
        let (events, _) = frame(
            &mut BasicInfoWindow::new(),
            &mut character,
            0,
            Some(Click::Right(x, y)),
        );
        assert!(matches!(events[..], [GameEvent::ToggleMinimap]));
        // only the map button has a right click
        let (x, y) = cell(0, WIN_H);
        let (events, _) = frame(
            &mut BasicInfoWindow::new(),
            &mut character,
            0,
            Some(Click::Right(x, y)),
        );
        assert!(events.is_empty());
    }

    #[test]
    fn the_menu_uses_classic_height_for_every_incoming_class() {
        for job in [0, 7, 4008, 4252] {
            let mut win = BasicInfoWindow::new();
            let mut character = Character::new();
            frame(&mut win, &mut character, job, Some(Click::Left(100.0, WIN_H + 4.0)));
            assert!(!win.is_menu_open());
        }
    }

    #[test]
    fn the_mini_button_minimizes_and_restores() {
        let mut win = BasicInfoWindow::new();
        let mut character = Character::new();
        let mini = Click::Left(WIN_W - 7.0, 8.0);
        frame(&mut win, &mut character, 0, Some(mini));
        assert_eq!(win.body_height(0), WIN_H_SMALL);
        frame(&mut win, &mut character, 0, Some(mini));
        assert_eq!(win.body_height(0), WIN_H);
    }
}
