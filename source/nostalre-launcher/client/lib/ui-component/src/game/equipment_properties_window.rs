//! Equipment Attributes (`CUIEquipmentPropertiesWnd`, the 2025 exe's 0x9c76c0): what the worn gear adds, in kRO's
//! eight tabs, Total and the Order table's seven categories (`docs/nostalre-kro-exe-equip-window.md`, "Equipment
//! Attributes" 7).
use crate::helper::scrollbar::{self, ScrollbarIds};
use crate::helper::tab_control::fitted_label;
use crate::helper::window_chrome::{
    SYS_BASE_OFF_TEX, SYS_BASE_ON_TEX, TITLEBAR_TEX, draw_sys_button, draw_textured_quad,
    draw_titlebar, fill,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::equipment_properties::WindowLine;
use nostalre_game::event::GameEvent;
use nostalre_resources::ui as art;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const EQUIP_PROPS_WINDOW_ID: WidgetId = WidgetId(6200);
const CLOSE_BTN_ID: WidgetId = WidgetId(6201);
const RESIZE_ID: WidgetId = WidgetId(6202);
const SCROLL_UP_ID: WidgetId = WidgetId(6203);
const SCROLL_DOWN_ID: WidgetId = WidgetId(6204);
const SCROLL_THUMB_ID: WidgetId = WidgetId(6205);
/// The tabs, 6210..6217, and the lines' label tooltips, 6220 on
const TAB_BASE_ID: u32 = 6210;
const LINE_BASE_ID: u32 = 6220;

/// Total and the seven categories
pub const TAB_COUNT: usize = 8;

/// The window: 300 wide, 308 high at first, only the height resizable to 470 in steps of a line (0x9c2de0, the
/// resizer 0x9c24c0)
const WIN_W: f32 = 300.0;
const TITLE_H: f32 = 17.0;
const LINE_H: f32 = 18.0;
const MIN_LINES: usize = 15;
const MAX_LINES: usize = 24;
/// The list (21, 17, 278, 270 at 15 lines); the bottom bar under it, 21 high
const LIST_X: f32 = 21.0;
const LIST_Y: f32 = 17.0;
const BOTTOM_BAR_H: f32 = 21.0;
/// The tabs at (3, 18 + 33 i), 18 × 34 (0x9c43dc..)
const TAB_X: f32 = 3.0;
const TAB_Y: f32 = 18.0;
const TAB_STEP: f32 = 33.0;
const TAB_SIZE: (f32, f32) = (18.0, 34.0);
/// A line: 265 wide; the value centred in (0, 4, 44, 12), the label from 49 in 216, a divider at 44 (0x9c5600)
const LINE_W: f32 = 265.0;
const VALUE_W: f32 = 44.0;
const LABEL_X: f32 = 49.0;
const LABEL_W: f32 = 216.0;
const TEXT_TOP: f32 = 4.0;
/// Scroll bar at (286, 17), resizer 13 × 13 at (286, h − 14), close at (286, 3)
const SCROLL_X: f32 = 286.0;
const RESIZER_SIZE: f32 = 13.0;
const CLOSE_POS: (f32, f32) = (286.0, 3.0);
const CLOSE_SIZE: f32 = 11.0;
const LEFT_STRIP_W: f32 = 20.0;
const LEFT_STRIP_TILE_H: f32 = 8.0;

/// colorchip.bmp's cells (0x9a5010, 0x9c7ec0): the frame's white and grey, the odd rows' and divider's blue
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const OUTLINE: [f32; 4] = [200.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0, 1.0];
const ROW_BLUE: [f32; 4] = [221.0 / 255.0, 231.0 / 255.0, 249.0 / 255.0, 1.0];
/// The value's blue, red after `{~}` (0x9c7b00: 0xff2a00, 0x3c00ff as 0x00BBGGRR); the label black
const VALUE_COLOR: [f32; 4] = [0.0, 42.0 / 255.0, 1.0, 1.0];
const NEGATIVE_COLOR: [f32; 4] = [1.0, 0.0, 60.0 / 255.0, 1.0];
const LABEL_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

const TABS: [[&str; 2]; TAB_COUNT] = [
    [
        art::EQUIP_PROPS_TAB_TOTAL_OFF,
        art::EQUIP_PROPS_TAB_TOTAL_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY1_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY1_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY2_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY2_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY3_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY3_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY4_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY4_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY5_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY5_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY6_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY6_ON,
    ],
    [
        art::EQUIP_PROPS_TAB_CATEGORY7_OFF,
        art::EQUIP_PROPS_TAB_CATEGORY7_ON,
    ],
];
/// What a tab says without the archive's art
const TAB_FALLBACK: [&str; TAB_COUNT] = ["T", "1", "2", "3", "4", "5", "6", "7"];

#[derive(Default)]
pub struct EquipmentPropertiesWindow {
    pub has_grf_textures: bool,
    open: bool,
    tab: usize,
    lines: [Vec<WindowLine>; TAB_COUNT],
    /// The current tab's labels cut to their box with "...", made once per fill
    fitted: Option<Vec<String>>,
    /// The first line shown; kept across fills and tabs (0x9c7f40), clamped
    scroll: usize,
    visible_lines: usize,
    resize_start: Option<usize>,
    /// What the lines were made from: the client runs the scripts again when it changes
    pub fingerprint: Option<u64>,
}

impl EquipmentPropertiesWindow {
    pub fn new() -> Self {
        Self {
            visible_lines: MIN_LINES,
            ..Default::default()
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The footer's button and Ctrl + Q: open, or close when open (0x8c056b, behaviour 161). kRO makes the window
    /// anew on each opening, on Total
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.tab = 0;
        self.scroll = 0;
        self.fingerprint = None;
        self.fitted = None;
    }

    pub fn set_lines(&mut self, lines: [Vec<WindowLine>; TAB_COUNT]) {
        self.lines = lines;
        self.fitted = None;
    }

    fn height(&self) -> f32 {
        LIST_Y + self.visible_lines as f32 * LINE_H + BOTTOM_BAR_H
    }

    fn list_h(&self) -> f32 {
        self.visible_lines as f32 * LINE_H
    }

    /// The frame (0x9a4fa0), the bottom bar, the left strip and the list's border (0x9c7cc0)
    fn draw_frame(&self, ui: &mut UiFrame, x: f32, y: f32, h: f32, grf: bool) {
        fill(ui, x, y, WIN_W, h, WHITE);
        // The 1-px outline, its corners cut
        fill(ui, x + 3.0, y, WIN_W - 6.0, 1.0, OUTLINE);
        fill(ui, x + 3.0, y + h - 1.0, WIN_W - 6.0, 1.0, OUTLINE);
        fill(ui, x, y + 3.0, 1.0, h - 6.0, OUTLINE);
        fill(ui, x + WIN_W - 1.0, y + 3.0, 1.0, h - 6.0, OUTLINE);
        let list_h = self.list_h();
        if grf {
            let bar_y = y + h - BOTTOM_BAR_H;
            draw_textured_quad(
                ui,
                x,
                bar_y,
                BOTTOM_BAR_H,
                BOTTOM_BAR_H,
                art::basic::BTNBAR_LEFT,
            );
            draw_textured_quad(
                ui,
                x + BOTTOM_BAR_H,
                bar_y,
                WIN_W - 2.0 * BOTTOM_BAR_H,
                BOTTOM_BAR_H,
                art::basic::BTNBAR_MID,
            );
            draw_textured_quad(
                ui,
                x + WIN_W - BOTTOM_BAR_H,
                bar_y,
                BOTTOM_BAR_H,
                BOTTOM_BAR_H,
                art::basic::BTNBAR_RIGHT,
            );
            let mut offset = 0.0;
            while offset < list_h + 1.0 {
                draw_textured_quad(
                    ui,
                    x,
                    y + LIST_Y + offset,
                    LEFT_STRIP_W,
                    LEFT_STRIP_TILE_H,
                    art::basic::ITEMWIN_LEFT,
                );
                offset += LEFT_STRIP_TILE_H;
            }
        }
        fill(ui, x + LEFT_STRIP_W, y + LIST_Y, 1.0, list_h + 1.0, OUTLINE);
        fill(
            ui,
            x + LEFT_STRIP_W,
            y + LIST_Y + list_h,
            WIN_W - LEFT_STRIP_W,
            1.0,
            OUTLINE,
        );
    }

    fn build_tabs(&mut self, ui: &mut UiFrame, x: f32, y: f32, grf: bool) {
        let (w, h) = TAB_SIZE;
        for (i, images) in TABS.iter().enumerate() {
            let rect = Rect::new(x + TAB_X, y + TAB_Y + TAB_STEP * i as f32, w, h);
            let response = ui.interact(WidgetId(TAB_BASE_ID + i as u32), rect);
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }
            // A tab answers on the press (0x9ae860)
            if response.hovered() && ui.ctx.mouse_clicked && self.tab != i {
                self.tab = i;
                self.fitted = None;
            }
            let current = self.tab == i;
            if grf {
                draw_textured_quad(ui, rect.x, rect.y, w, h, images[usize::from(current)]);
            } else {
                let color = if current { ROW_BLUE } else { WHITE };
                fill(ui, rect.x, rect.y, w, h, OUTLINE);
                fill(ui, rect.x + 1.0, rect.y + 1.0, w - 2.0, h - 2.0, color);
                ui.text(rect.x + 5.0, rect.y + 20.0, TAB_FALLBACK[i], LABEL_COLOR);
            }
        }
    }

    fn build_list(&mut self, ui: &mut UiFrame, x: f32, y: f32) {
        let list_x = x + LIST_X;
        let list_y = y + LIST_Y;
        let list_h = self.list_h();
        let visible = self.visible_lines;
        let lines = &self.lines[self.tab];
        let max_scroll = lines.len().saturating_sub(visible);
        let list_rect = Rect::new(list_x, list_y, LINE_W, list_h);
        self.scroll = scrollbar::scrollbar(
            ui,
            ScrollbarIds {
                up: SCROLL_UP_ID,
                down: SCROLL_DOWN_ID,
                thumb: SCROLL_THUMB_ID,
            },
            self.scroll,
            visible,
            max_scroll,
            list_rect,
            x + SCROLL_X,
            list_y,
            list_h,
        );
        let scroll = self.scroll;
        let fitted = self.fitted.get_or_insert_with(|| {
            lines
                .iter()
                .map(|line| fitted_label(&line.label, LABEL_W, |t| ui.atlas.measure_text(t)))
                .collect()
        });
        let ascent = ui.atlas.ascent;
        let shown = lines
            .iter()
            .zip(fitted.iter())
            .enumerate()
            .skip(scroll)
            .take(visible);
        for (row, (line, label)) in shown {
            let top = list_y + (row - scroll) as f32 * LINE_H;
            let background = if row % 2 == 0 { WHITE } else { ROW_BLUE };
            fill(ui, list_x, top, LINE_W, LINE_H, background);
            fill(ui, list_x + VALUE_W, top, 1.0, LINE_H, ROW_BLUE);
            let value_color = if line.negative {
                NEGATIVE_COLOR
            } else {
                VALUE_COLOR
            };
            let value_x = list_x + ((VALUE_W - ui.atlas.measure_text(&line.value)) * 0.5).floor();
            ui.text(value_x, top + TEXT_TOP + ascent, &line.value, value_color);
            ui.text(
                list_x + LABEL_X,
                top + TEXT_TOP + ascent,
                label,
                LABEL_COLOR,
            );
            // A cut label shows whole on hover (0x9adcf0)
            if label.len() != line.label.len() {
                let rect = Rect::new(list_x + LABEL_X, top, LABEL_W, LINE_H);
                if ui
                    .interact(WidgetId(LINE_BASE_ID + (row - scroll) as u32), rect)
                    .hovered()
                {
                    ui.tooltip(rect.x, rect.y + LINE_H + 2.0, &line.label);
                }
            }
        }
    }
}

impl Window for EquipmentPropertiesWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, self.height())
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = vec![
            TITLEBAR_TEX,
            SYS_BASE_OFF_TEX,
            SYS_BASE_ON_TEX,
            art::basic::SYS_CLOSE_OFF,
            art::basic::SYS_CLOSE_ON,
            art::basic::BTNBAR_LEFT,
            art::basic::BTNBAR_MID,
            art::basic::BTNBAR_RIGHT,
            art::basic::ITEMWIN_LEFT,
            art::BTN_RESIZE,
        ];
        paths.extend(TABS.iter().flatten());
        paths.extend(scrollbar::grf_texture_paths());
        paths
    }
}

impl InGameWindow for EquipmentPropertiesWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.toggle();
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;

        let h = self.height();
        let win = ui.window_at(EQUIP_PROPS_WINDOW_ID, WIN_W, h, TITLE_H, 0.0, 0.0);
        let (x, y) = (win.x, win.y);
        ui.interact(EQUIP_PROPS_WINDOW_ID, Rect::new(x, y, WIN_W, h));

        self.draw_frame(ui, x, y, h, grf);
        draw_titlebar(ui, x, y, WIN_W, TITLE_H, grf);
        // MSI 4029 at (18, 0) in an 18-high box, black with a white shadow at (+1, +1)
        let title_y = y + ((18.0 - ui.atlas.line_height) * 0.5).floor() + ui.atlas.ascent;
        ui.text(x + 19.0, title_y + 1.0, "Equipment Attributes", WHITE);
        ui.text(x + 18.0, title_y, "Equipment Attributes", LABEL_COLOR);

        let close_rect = Rect::new(x + CLOSE_POS.0, y + CLOSE_POS.1, CLOSE_SIZE, CLOSE_SIZE);
        let close = ui.interact(CLOSE_BTN_ID, close_rect);
        if close.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (CLOSE_SIZE, CLOSE_SIZE),
            close.hovered(),
            grf,
            art::basic::SYS_CLOSE_ON,
            art::basic::SYS_CLOSE_OFF,
            Some('x'),
        );
        if close.clicked() {
            self.toggle();
            ui.has_grf_textures = prev_grf;
            return Vec::new();
        }

        self.build_tabs(ui, x, y, grf);
        self.build_list(ui, x, y);

        let resizer = Rect::new(
            x + SCROLL_X,
            y + h - RESIZER_SIZE - 1.0,
            RESIZER_SIZE,
            RESIZER_SIZE,
        );
        if grf {
            draw_textured_quad(
                ui,
                resizer.x,
                resizer.y,
                RESIZER_SIZE,
                RESIZER_SIZE,
                art::BTN_RESIZE,
            );
        }
        let resize = ui.resize_handle(RESIZE_ID, resizer);
        if resize.started {
            self.resize_start = Some(self.visible_lines);
        }
        if resize.dragging
            && let Some(start) = self.resize_start
        {
            let lines = (start as f32 + resize.delta_y / LINE_H).round() as i32;
            self.visible_lines = lines.clamp(MIN_LINES as i32, MAX_LINES as i32) as usize;
        }

        ui.has_grf_textures = prev_grf;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(value: &str, label: &str) -> WindowLine {
        WindowLine {
            value: value.into(),
            label: label.into(),
            negative: false,
        }
    }

    #[test]
    fn opens_on_total_and_toggles() {
        let mut window = EquipmentPropertiesWindow::new();
        window.toggle();
        assert!(window.is_open());
        window.tab = 3;
        window.toggle();
        window.toggle();
        assert_eq!(window.tab, 0);
        assert_eq!(window.window_size(), (300.0, 308.0));
    }

    #[test]
    fn new_lines_drop_the_cut_labels() {
        let mut window = EquipmentPropertiesWindow::new();
        window.fitted = Some(vec!["old".into()]);
        window.set_lines(std::array::from_fn(|_| vec![line("+29", "ATK")]));
        assert!(window.fitted.is_none());
        assert_eq!(window.lines[0][0].label, "ATK");
    }
}
